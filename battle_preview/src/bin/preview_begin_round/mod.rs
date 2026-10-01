use std::{
    collections::{HashMap, VecDeque},
    env, fs, io,
    path::{Path, PathBuf},
};

use battle::engine::{runtime::BattleRuntime, skill::effect::catalog};
use battle_preview::{
    array_len_diff_lines, begin_round_inputs, build_metadata_source, canonical_comparison,
    expand_compressed_fight_steps, first_diff_path, normalize_live_json, opening_determinism,
    preview_attributes, preview_output_text, render_json_with_capture_conventions,
    seed_round_determinism, tower_plan_id,
};
use sonettobuf::{BeginRoundReply, BeginRoundRequest, Fight, FightRound, FightStep};

const TRACED_DAMAGE_EFFECT_TYPES: [i32; 8] = [
    sonettobuf::effect_type_enum::EffectType::Damage as i32,
    sonettobuf::effect_type_enum::EffectType::Crit as i32,
    sonettobuf::effect_type_enum::EffectType::Origindamage as i32,
    sonettobuf::effect_type_enum::EffectType::Origincrit as i32,
    sonettobuf::effect_type_enum::EffectType::Additionaldamage as i32,
    sonettobuf::effect_type_enum::EffectType::Additionaldamagecrit as i32,
    sonettobuf::effect_type_enum::EffectType::Nuodikarandomattack as i32,
    sonettobuf::effect_type_enum::EffectType::Nuodikateamattack as i32,
];

fn main() -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name("begin-round-preview".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| io::Error::other("begin-round preview thread panicked"))?
}

fn run() -> anyhow::Result<()> {
    let db = init_config()?;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let input_root = root.join("battles");
    let output_root = root.join("battles_gen");
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let resync = take_flag(&mut args, "--resync");
    let inputs = begin_round_inputs(&input_root, args)?;

    for (through, inputs) in battle_groups(inputs) {
        match replay_rounds(db, &through, resync) {
            Ok(run) => {
                for input in inputs {
                    if let Err(error) = report_input(&input_root, &output_root, &input, &run) {
                        println!("{} round=ERROR {error:#}", input.display());
                    }
                }
            }
            Err(error) => {
                for input in inputs {
                    println!("{} round=ERROR {error:#}", input.display());
                }
            }
        }
    }

    Ok(())
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg != flag);
    args.len() != before
}

// One replay per battle directory, through the latest requested round.
fn battle_groups(inputs: Vec<PathBuf>) -> Vec<(PathBuf, Vec<PathBuf>)> {
    let mut groups = std::collections::BTreeMap::<PathBuf, Vec<(i32, PathBuf)>>::new();
    for input in inputs {
        let index = round_index(&input).unwrap_or(i32::MAX);
        groups
            .entry(input.parent().map(Path::to_path_buf).unwrap_or_default())
            .or_default()
            .push((index, input));
    }
    groups
        .into_values()
        .filter_map(|mut inputs| {
            inputs.sort();
            let through = inputs.last()?.1.clone();
            Some((
                through,
                inputs.into_iter().map(|(_, input)| input).collect(),
            ))
        })
        .collect()
}

fn report_input(
    input_root: &Path,
    output_root: &Path,
    input: &Path,
    run: &ReplayRun,
) -> anyhow::Result<()> {
    let index = round_index(input)?;
    let Some(outcome) = run.outcomes.iter().find(|outcome| outcome.index == index) else {
        let reason = match run.outcomes.last() {
            Some(RoundOutcome {
                index,
                result: Err(error),
                ..
            }) => format!("round {index}: {error}"),
            _ => run
                .stopped
                .clone()
                .unwrap_or_else(|| "round was not replayed".to_owned()),
        };
        println!("{} round=BLOCKED {reason}", input.display());
        return Ok(());
    };
    let generated_round = match &outcome.result {
        Ok(round) => round.clone(),
        Err(error) => {
            println!("{} round=ERROR {error}", input.display());
            return Ok(());
        }
    };
    let original_text = fs::read_to_string(input)?;
    let original = captured_original(input)?;
    if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage) {
        report_damage_comparison(&generated_round, &captured_round(input)?);
    }
    let generated = BeginRoundReply {
        round: Some(generated_round),
    };
    let generated_value = serde_json::to_value(&generated)?;
    let output_value = render_json_with_capture_conventions(&generated_value, &original);
    let (generated_compare, original_compare) =
        canonical_comparison(generated_value, original.clone());
    let round_matches = generated_compare.get("round") == original_compare.get("round");
    if !round_matches
        && let (Some(generated_round), Some(original_round)) = (
            generated_compare.get("round"),
            original_compare.get("round"),
        )
        && let Some(path) = first_diff_path(generated_round, original_round, "/round")
    {
        eprintln!("  first round diff: {path}");
        for line in array_len_diff_lines(generated_round, original_round, &path, "/round") {
            eprintln!("    {line}");
        }
    }
    let output = output_path(input_root, output_root, input)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        preview_output_text(&output_value, &original, original_text)?,
    )?;
    println!(
        "{} round={}{} build={}",
        output.display(),
        if round_matches { "MATCH" } else { "DIFF" },
        if outcome.resynced { " resynced" } else { "" },
        build_metadata_source(input)?,
    );
    Ok(())
}

fn init_config() -> anyhow::Result<&'static config::GameDB> {
    let data = env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap())?;
    Ok(config::configs::get())
}

fn captured_start_reply(round_path: &Path) -> anyhow::Result<serde_json::Value> {
    let dungeon_path = round_path.with_file_name("StartDungeonReply.json");
    let path = if dungeon_path.exists() {
        dungeon_path
    } else {
        round_path.with_file_name("StartTowerBattleReply.json")
    };
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    if let Some(start) = value.get("startDungeonReply").cloned() {
        value = start;
    }
    normalize_live_json(&mut value);
    Ok(value)
}

fn captured_original(path: &Path) -> anyhow::Result<serde_json::Value> {
    let mut original: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    expand_compressed_fight_steps(&mut original)?;
    Ok(original)
}

#[cfg(all(test, feature = "private-fixtures"))]
fn generate_reply(
    db: &'static config::GameDB,
    path: &Path,
) -> anyhow::Result<(BeginRoundReply, serde_json::Value)> {
    let original = captured_original(path)?;
    let round = replay_to_round(db, path)?;

    Ok((BeginRoundReply { round: Some(round) }, original))
}

#[cfg(all(test, feature = "private-fixtures"))]
fn replay_to_round(db: &'static config::GameDB, path: &Path) -> anyhow::Result<FightRound> {
    let outcome = replay_rounds(db, path, false)?
        .outcomes
        .pop()
        .ok_or_else(|| io::Error::other(format!("{} has no replayed round", path.display())))?;
    outcome
        .result
        .map_err(|error| io::Error::other(error).into())
}

struct RoundOutcome {
    index: i32,
    result: Result<FightRound, String>,
    resynced: bool,
}

struct ReplayRun {
    outcomes: Vec<RoundOutcome>,
    stopped: Option<String>,
}

/// Replays captured requests through `BattleRuntime` from the captured start-state fixture,
/// once, through the round at `path`. Later captured replies are comparison evidence; with
/// `resync` they also reset observed HP, ex points, and the hand between rounds so later
/// rounds can run past an earlier divergence. The first failing round ends the replay.
fn replay_rounds(
    db: &'static config::GameDB,
    path: &Path,
    resync: bool,
) -> anyhow::Result<ReplayRun> {
    let round_index = round_index(path)?;
    let value = captured_start_reply(path)?;
    let fight = value.get("fight").cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has no captured start fight", path.display()),
        )
    })?;
    let fight: Fight = serde_json::from_value(fight)?;
    let captured_start_round: FightRound = serde_json::from_value(
        value
            .get("round")
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "capture has no round"))?,
    )?;
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, path)?;
    let tower_rule_skills = tower_plan_id(path)
        .map(|plan_id| battle::tower::system_plan_rule_skills(db, &fight, plan_id))
        .unwrap_or_default();
    let opening_determinism = opening_determinism(db, &fight, &captured_start_round);
    let mut runtime = BattleRuntime::new_with_attributes(
        battle::catalog::BattleCatalog::new(db),
        fight,
        ex_attributes,
        sp_attributes,
    );
    runtime
        .inherit_absorb_hurt_map_layout(&captured_start_round)
        .map_err(anyhow::Error::msg)?;
    runtime.extend_battle_rule_skills(tower_rule_skills);
    let start_round = runtime
        .start_round_with_determinism(opening_determinism)
        .map_err(io::Error::other)?;
    if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage) {
        eprintln!("  start-round damage:");
        report_damage_comparison(&start_round, &captured_start_round);
    }
    let mut resynced = false;
    if resync {
        resynced |= resync_round(&mut runtime, 0, &captured_start_round)?;
    }
    let mut previous_captured_round = captured_start_round;
    let mut outcomes = Vec::new();
    let mut stopped = None;
    replay_cloth_input(path, 0, &mut runtime)?;

    for index in round_indices(path, round_index)? {
        let (request_name, reply_name) = if uses_legacy_round_names(path) {
            (
                format!("BeginRoundRequest_{index}.json"),
                format!("BeginRoundReply_{index}.json"),
            )
        } else {
            (
                format!("begin_round_{index}_request.json"),
                format!("begin_round_{index}.json"),
            )
        };
        let request_path = path.with_file_name(request_name);
        let request = begin_round_request(&request_path)?;
        let captured = captured_round(&path.with_file_name(reply_name))?;
        validate_captured_round_continuity(&previous_captured_round, &captured)?;
        report_rule_issues(&captured);
        replay_cloth_input(path, index, &mut runtime)?;
        seed_round_determinism(&mut runtime, catalog::global(), &captured);
        let result = runtime.advance_round(request);
        let failed = result.is_err();
        outcomes.push(RoundOutcome {
            index,
            result,
            resynced,
        });
        if failed {
            break;
        }
        if resync {
            match resync_round(&mut runtime, index, &captured) {
                Ok(changed) => resynced |= changed,
                Err(error) => {
                    stopped = Some(format!("resync after round {index}: {error}"));
                    break;
                }
            }
        }
        previous_captured_round = captured;
    }

    Ok(ReplayRun { outcomes, stopped })
}

fn resync_round(
    runtime: &mut BattleRuntime,
    index: i32,
    captured: &FightRound,
) -> anyhow::Result<bool> {
    let changes = runtime
        .resync_observed_state(captured)
        .map_err(io::Error::other)?;
    for change in &changes {
        eprintln!("  resync after round {index}: {change}");
    }
    Ok(!changes.is_empty())
}

fn replay_cloth_input(
    path: &Path,
    round_index: i32,
    runtime: &mut BattleRuntime,
) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    for input in cloth_input_paths(parent, round_index)? {
        let mut request: serde_json::Value = serde_json::from_str(&fs::read_to_string(&input)?)?;
        normalize_live_json(&mut request);
        let request = serde_json::from_value(request)?;
        runtime
            .use_cloth_skill(request)
            .ok_or_else(|| io::Error::other(format!("invalid cloth input: {}", input.display())))?;
    }
    Ok(())
}

fn cloth_input_paths(parent: &Path, round_index: i32) -> io::Result<Vec<PathBuf>> {
    let base_name = format!("UseClothSkillRequest_{round_index}");
    let mut inputs = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_cloth_input(path, &base_name))
        .collect::<Vec<_>>();
    inputs.sort();
    Ok(inputs)
}

fn is_cloth_input(path: &Path, base_name: &str) -> bool {
    path.file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name == base_name
                || name
                    .strip_prefix(base_name)
                    .is_some_and(|suffix| suffix.starts_with('_'))
        })
        && path
            .extension()
            .is_some_and(|extension| extension == "json")
}

fn report_rule_issues(round: &FightRound) {
    let mut skill_ids = nested_steps(round)
        .filter_map(|step| step.act_id)
        .filter(|skill_id| *skill_id != 0)
        .collect::<Vec<_>>();
    skill_ids.sort_unstable();
    skill_ids.dedup();

    for skill_id in skill_ids {
        for issue in catalog::global().issues(skill_id) {
            eprintln!(
                "  unsupported rule: skill={skill_id} effect={} slot={} opcode={:?} type={:?} reason={:?} raw={:?}",
                issue.effect_id, issue.slot, issue.opcode, issue.type_name, issue.reason, issue.raw,
            );
        }
    }
}

fn nested_steps(round: &FightRound) -> impl Iterator<Item = &FightStep> {
    let mut pending = round.fight_step.iter().rev().collect::<Vec<_>>();
    std::iter::from_fn(move || {
        let step = pending.pop()?;
        pending.extend(
            step.act_effect
                .iter()
                .rev()
                .filter_map(|effect| effect.fight_step.as_ref()),
        );
        Some(step)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DamageIdentity {
    skill_id: i32,
    source_uid: i64,
    target_uid: i64,
    effect_type: i32,
    config_effect: i32,
    buff_act_id: i32,
}

fn damage_observations(round: &FightRound) -> Vec<(DamageIdentity, (i32, bool))> {
    nested_steps(round)
        .flat_map(|step| {
            step.act_effect.iter().filter_map(move |effect| {
                let effect_type = effect.effect_type?;
                TRACED_DAMAGE_EFFECT_TYPES
                    .contains(&effect_type)
                    .then_some((
                        DamageIdentity {
                            skill_id: step.act_id.unwrap_or_default(),
                            source_uid: step.from_id.unwrap_or_default(),
                            target_uid: effect.target_id.unwrap_or_default(),
                            effect_type,
                            config_effect: effect.config_effect.unwrap_or_default(),
                            buff_act_id: effect.buff_act_id.unwrap_or_default(),
                        },
                        (
                            effect.effect_num.unwrap_or_default(),
                            effect
                                .hurt_info
                                .as_ref()
                                .and_then(|info| info.career_restraint)
                                .unwrap_or_default(),
                        ),
                    ))
            })
        })
        .collect()
}

fn report_damage_comparison(generated: &FightRound, captured: &FightRound) {
    for (label, round) in [("generated", generated), ("captured", captured)] {
        for effect in nested_steps(round)
            .flat_map(|step| &step.act_effect)
            .filter(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Twinsupcount as i32)
            })
        {
            eprintln!(
                "  conduit consumed {label}={} config={}",
                effect.effect_num.unwrap_or_default(),
                effect.config_effect.unwrap_or_default()
            );
        }
    }
    let mut captured = damage_observations(captured).into_iter().fold(
        HashMap::<DamageIdentity, VecDeque<(i32, bool)>>::new(),
        |mut amounts, (identity, observation)| {
            amounts.entry(identity).or_default().push_back(observation);
            amounts
        },
    );
    for (identity, (generated, generated_restrained)) in damage_observations(generated) {
        let captured = captured.get_mut(&identity).and_then(VecDeque::pop_front);
        match captured {
            Some((captured, captured_restrained)) => {
                let delta = generated - captured;
                eprintln!(
                    "  damage parity identity={identity:?} generated={generated} captured={captured} delta={delta} restrained={generated_restrained}/{captured_restrained} earliest_observable_divergence={}",
                    if delta == 0 { "none" } else { "final_effect" },
                );
            }
            None => eprintln!(
                "  damage parity identity={identity:?} generated={generated} captured=missing"
            ),
        }
    }
    for (identity, amounts) in captured {
        for (captured, restrained) in amounts {
            eprintln!(
                "  damage parity identity={identity:?} generated=missing captured={captured} restrained={restrained}"
            );
        }
    }
}

fn begin_round_request(path: &Path) -> anyhow::Result<BeginRoundRequest> {
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    normalize_live_json(&mut value);
    Ok(serde_json::from_value(value)?)
}

fn captured_round(path: &Path) -> anyhow::Result<FightRound> {
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    expand_compressed_fight_steps(&mut value)?;
    normalize_live_json(&mut value);
    let round = value.get("round").cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has no round", path.display()),
        )
    })?;
    Ok(serde_json::from_value(round)?)
}

fn validate_captured_round_continuity(previous: &FightRound, next: &FightRound) -> io::Result<()> {
    match (previous.cur_round, next.cur_round) {
        (Some(previous), Some(next_round))
            if i64::from(next_round) != i64::from(previous) + 1
                && !(next_round == previous && next.is_finish == Some(true)) =>
        {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "captured round sequence is misaligned: previous round {previous}, next round {next_round}"
                ),
            ))
        }
        _ => Ok(()),
    }
}

fn round_index(path: &Path) -> anyhow::Result<i32> {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} has no file stem", path.display()),
            )
        })?;
    let index = stem
        .strip_prefix("begin_round_")
        .or_else(|| stem.strip_prefix("BeginRoundReply_"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a begin-round capture", path.display()),
            )
        })?;
    Ok(index.parse()?)
}

fn uses_legacy_round_names(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.starts_with("BeginRoundReply_"))
}

fn round_indices(path: &Path, through: i32) -> anyhow::Result<Vec<i32>> {
    let prefix = if uses_legacy_round_names(path) {
        "BeginRoundReply_"
    } else {
        "begin_round_"
    };
    let mut indices = fs::read_dir(path.parent().unwrap_or_else(|| Path::new(".")))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .path()
                .file_stem()?
                .to_str()?
                .strip_prefix(prefix)?
                .parse()
                .ok()
        })
        .filter(|index| *index <= through)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    Ok(indices)
}

fn output_path(input_root: &Path, output_root: &Path, input: &Path) -> anyhow::Result<PathBuf> {
    Ok(match input.strip_prefix(input_root) {
        Ok(path) => output_root.join(path),
        Err(_) => battle_preview::external_output_path(output_root, &fs::canonicalize(input)?),
    })
}

#[cfg(test)]
mod tests;
