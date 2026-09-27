use std::{
    collections::HashSet,
    env, fs, io,
    path::{Path, PathBuf},
};

use battle::engine::runtime::BattleRuntime;
use battle_preview::{
    battle_inputs, canonical_comparison, comparable_json, first_diff_path, normalize_live_json,
    preview_attributes, preview_output_text, render_json_with_capture_conventions, tower_plan_id,
};
use sonettobuf::{CardInfoPush, Fight, StartDungeonReply};

fn main() -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name("start-dungeon-preview".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| io::Error::other("start-dungeon preview thread panicked"))?
}

fn run() -> anyhow::Result<()> {
    let db = init_config()?;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let input_root = root.join("battles");
    let output_root = root.join("battles_gen");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let inputs = start_inputs(&input_root, args)?;
    let mut outputs = HashSet::new();

    for input in inputs {
        let output = output_path(&input_root, &output_root, &input, &mut outputs)?;
        let original_text = fs::read_to_string(&input)?;
        let (generated, cards, original) = generate_reply(db, &input)?;
        let generated_value = serde_json::to_value(&generated)?;
        let captured = captured_start_reply(&original);
        let output_value = render_json_with_capture_conventions(&generated_value, captured);
        let (generated_compare, original_compare) =
            canonical_comparison(generated_value, captured.clone());
        let fight_matches = generated_compare.get("fight") == original_compare.get("fight");
        let round_matches = generated_compare.get("round") == original_compare.get("round");
        let card_matches = compare_card_push(&input, cards)?;
        if !round_matches
            && let (Some(generated_round), Some(original_round)) = (
                generated_compare.get("round"),
                original_compare.get("round"),
            )
            && let Some(path) = first_diff_path(generated_round, original_round, "/round")
        {
            eprintln!("  first round diff: {path}");
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let output_value = if original.get("startDungeonReply").is_some() {
            serde_json::json!({ "startDungeonReply": output_value })
        } else {
            output_value
        };
        fs::write(
            &output,
            preview_output_text(&output_value, &original, original_text)?,
        )?;
        println!(
            "{} fight={} round={} cards={}",
            output.display(),
            if fight_matches { "MATCH" } else { "DIFF" },
            if round_matches { "MATCH" } else { "DIFF" },
            card_matches.map_or("N/A", |matches| if matches { "MATCH" } else { "DIFF" }),
        );
    }

    Ok(())
}

fn init_config() -> anyhow::Result<&'static config::GameDB> {
    let data = env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap())?;
    Ok(config::configs::get())
}

fn start_inputs(root: &Path, args: Vec<String>) -> anyhow::Result<Vec<PathBuf>> {
    if args.is_empty() {
        let mut inputs = battle_inputs(root, Vec::new(), "StartDungeonReply.json")?;
        inputs.extend(battle_inputs(
            root,
            Vec::new(),
            "StartTowerBattleReply.json",
        )?);
        inputs.sort();
        return Ok(inputs);
    }

    Ok(args
        .into_iter()
        .map(|arg| {
            let directory = root.join(&arg);
            ["StartDungeonReply.json", "StartTowerBattleReply.json"]
                .into_iter()
                .map(|name| directory.join(name))
                .find(|path| path.exists())
                .unwrap_or_else(|| PathBuf::from(arg))
        })
        .collect())
}

fn captured_start_reply(value: &serde_json::Value) -> &serde_json::Value {
    value.get("startDungeonReply").unwrap_or(value)
}

fn generate_reply(
    db: &'static config::GameDB,
    path: &Path,
) -> anyhow::Result<(StartDungeonReply, CardInfoPush, serde_json::Value)> {
    let original: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut value = captured_start_reply(&original).clone();
    normalize_live_json(&mut value);
    let fight = value.get("fight").cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has no fight", path.display()),
        )
    })?;
    let fight: Fight = serde_json::from_value(fight)?;
    let tower_rule_skills = tower_plan_id(path)
        .map(|plan_id| battle::tower::system_plan_rule_skills(db, &fight, plan_id))
        .unwrap_or_default();
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, path)?;
    let mut runtime = BattleRuntime::new_with_attributes(
        battle::catalog::BattleCatalog::new(db),
        fight,
        ex_attributes,
        sp_attributes,
    );
    runtime.extend_battle_rule_skills(tower_rule_skills);
    runtime.start_round().map_err(io::Error::other)?;

    Ok((
        battle::dungeon::start_reply(&runtime),
        runtime.card_info_push(),
        original,
    ))
}

fn compare_card_push(path: &Path, generated: CardInfoPush) -> anyhow::Result<Option<bool>> {
    let capture = path.with_file_name("CardInfoPush_1.json");
    if !capture.exists() {
        return Ok(None);
    }
    let captured = comparable_json(serde_json::from_str(&fs::read_to_string(capture)?)?);
    let generated = comparable_json(serde_json::to_value(generated)?);
    let matches = generated == captured;
    if !matches && let Some(path) = first_diff_path(&generated, &captured, "/cardInfoPush") {
        eprintln!("  first card push diff: {path}");
        if path.ends_with(".len") {
            let field = path
                .trim_start_matches("/cardInfoPush/")
                .trim_end_matches(".len");
            let generated_len = generated
                .get(field)
                .and_then(serde_json::Value::as_array)
                .map(Vec::len);
            let captured_len = captured
                .get(field)
                .and_then(serde_json::Value::as_array)
                .map(Vec::len);
            eprintln!("  card push {field} generated={generated_len:?} captured={captured_len:?}");
            if field == "cardGroup" {
                let summary = |value: &serde_json::Value| {
                    value
                        .get("cardGroup")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|card| {
                            (
                                card.get("uid").and_then(serde_json::Value::as_i64),
                                card.get("skillId").and_then(serde_json::Value::as_i64),
                                card.get("tempCard").and_then(serde_json::Value::as_bool),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                eprintln!("  generated cards={:?}", summary(&generated));
                eprintln!("  captured cards={:?}", summary(&captured));
            }
        }
    }
    Ok(Some(matches))
}

fn output_path(
    input_root: &Path,
    output_root: &Path,
    input: &Path,
    outputs: &mut HashSet<String>,
) -> anyhow::Result<PathBuf> {
    let canonical_input = fs::canonicalize(input)?;
    let relative = fs::canonicalize(input_root)
        .ok()
        .and_then(|root| canonical_input.strip_prefix(root).ok().map(PathBuf::from));
    let output = match relative {
        Some(path) => output_root.join(path),
        None => output_root
            .join(
                canonical_input
                    .parent()
                    .and_then(|path| path.file_name())
                    .unwrap_or_default(),
            )
            .join(canonical_input.file_name().unwrap_or_default()),
    };
    let collision_key = if cfg!(windows) {
        output.to_string_lossy().to_lowercase()
    } else {
        output.to_string_lossy().into_owned()
    };
    if !outputs.insert(collision_key) {
        anyhow::bail!("multiple inputs map to {}", output.display());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output_test_paths(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "enigma-start-preview-{label}-{}",
            std::process::id()
        ));
        let input_root = root.join("fixtures/battles");
        let output_root = root.join("fixtures/battles_gen");
        fs::create_dir_all(&input_root).unwrap();
        (root, input_root, output_root)
    }

    fn write_input(root: &Path, capture: &str, battle: &str) -> PathBuf {
        let input = root
            .join(capture)
            .join(battle)
            .join("StartDungeonReply.json");
        fs::create_dir_all(input.parent().unwrap()).unwrap();
        fs::write(&input, b"capture").unwrap();
        input
    }

    #[test]
    fn external_output_collisions_fail_loudly() {
        let (root, input_root, output_root) = output_test_paths("collision");
        let first = write_input(&root, "capture-a", "Battle1");
        let second = write_input(&root, "capture-b", "Battle1");
        let mut outputs = HashSet::new();
        output_path(&input_root, &output_root, &first, &mut outputs).unwrap();

        assert!(
            output_path(&input_root, &output_root, &second, &mut outputs)
                .unwrap_err()
                .to_string()
                .contains("multiple inputs map")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn external_output_collision_check_is_case_insensitive_on_windows() {
        let (root, input_root, output_root) = output_test_paths("case-collision");
        let first = write_input(&root, "capture-a", "Battle1");
        let second = write_input(&root, "capture-b", "battle1");
        let mut outputs = HashSet::new();
        output_path(&input_root, &output_root, &first, &mut outputs).unwrap();

        assert!(output_path(&input_root, &output_root, &second, &mut outputs).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parent_traversal_cannot_escape_the_output_root() {
        let (root, input_root, output_root) = output_test_paths("parent-traversal");
        let input = write_input(&input_root, "", "Battle1");
        let traversing = input_root
            .join("..")
            .join("battles/Battle1/StartDungeonReply.json");
        let output =
            output_path(&input_root, &output_root, &traversing, &mut HashSet::new()).unwrap();

        assert!(output.starts_with(&output_root));
        assert_ne!(fs::canonicalize(input).unwrap(), output);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_input_does_not_require_the_fixture_root() {
        let root = std::env::temp_dir().join(format!(
            "enigma-start-preview-missing-root-{}",
            std::process::id()
        ));
        let input = write_input(&root, "capture", "Battle1");
        let output_root = root.join("generated");

        assert!(
            output_path(
                &root.join("missing-fixtures"),
                &output_root,
                &input,
                &mut HashSet::new(),
            )
            .unwrap()
            .starts_with(output_root)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
