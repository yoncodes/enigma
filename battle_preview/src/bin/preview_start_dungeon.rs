use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use battle::engine::runtime::BattleRuntime;
use battle_preview::{
    battle_inputs, canonical_comparison, captured_opening_determinism, comparable_json,
    first_diff_path, normalize_live_json, preview_attributes, preview_output_text,
    render_json_with_capture_conventions, tower_plan_id,
};
use sonettobuf::{CardInfoPush, Fight, FightRound, StartDungeonReply};

fn main() -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name("start-dungeon-preview".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| io::Error::other("start-dungeon preview thread panicked"))?
}

fn run() -> anyhow::Result<()> {
    init_config()?;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let input_root = root.join("battles");
    let output_root = root.join("battles_gen");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let inputs = start_inputs(&input_root, args)?;

    for input in inputs {
        let original_text = fs::read_to_string(&input)?;
        let (generated, cards, original) = generate_reply(&input)?;
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
        let output = output_path(&input_root, &output_root, &input)?;
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

fn init_config() -> anyhow::Result<()> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json");
    config::init(data.to_str().unwrap())?;
    Ok(())
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
    let captured_round: FightRound = serde_json::from_value(
        value
            .get("round")
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "capture has no round"))?,
    )?;
    let tower_rule_skills = tower_plan_id(path)
        .map(|plan_id| {
            battle::tower::system_plan_rule_skills(config::configs::get(), &fight, plan_id)
        })
        .unwrap_or_default();
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, path)?;
    let opening_determinism = captured_opening_determinism(&fight, &captured_round);
    let mut runtime = BattleRuntime::new_with_attributes(fight, ex_attributes, sp_attributes);
    runtime.extend_battle_rule_skills(tower_rule_skills);
    runtime
        .start_round_with_determinism(opening_determinism)
        .map_err(io::Error::other)?;

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

fn output_path(input_root: &Path, output_root: &Path, input: &Path) -> anyhow::Result<PathBuf> {
    Ok(output_root.join(input.strip_prefix(input_root)?))
}
