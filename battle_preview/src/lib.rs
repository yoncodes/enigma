use std::{
    fs,
    path::{Path, PathBuf},
};

use battle::engine::runtime::determinism::RoundDeterminism;
use sonettobuf::{Fight, FightRound, StartDungeonRequest};

mod attributes;
mod compression;
mod normalize;

pub use attributes::preview_attributes;
pub use compression::expand_compressed_fight_steps;
pub use normalize::normalize_live_json;

/// Replays the captured opening RNG decisions through the engine's validated
/// card candidates instead of treating the captured hand as authoritative state.
pub fn captured_opening_determinism(fight: &Fight, round: &FightRound) -> RoundDeterminism {
    let mut determinism =
        RoundDeterminism::with_seed(fight.battle_id.unwrap_or_default().max(0) as u64);
    let draws = round
        .team_a_cards1
        .iter()
        .filter(|card| !card.temp_card.unwrap_or_default())
        .cloned()
        .collect::<Vec<_>>();
    let hand_size = battle::engine::manager::card::hand_size(fight);
    let player_candidates = battle::engine::manager::card::pool::player_candidate_pool(fight);
    let enemies = battle::engine::manager::card::pool::active_enemy_entities(fight);
    let enemy_count = enemies.len();
    let ai_candidates = enemies
        .into_iter()
        .flat_map(|entity| {
            entity
                .skill_group1
                .iter()
                .chain(&entity.skill_group2)
                .copied()
                .chain(entity.ex_skill)
                .filter_map(|skill_id| {
                    battle::engine::manager::card::pool::card_for(entity, Some(skill_id))
                })
        })
        .collect::<Vec<_>>();
    let valid_identity = |captured: &sonettobuf::CardInfo, candidates: &[sonettobuf::CardInfo]| {
        candidates.iter().any(|candidate| {
            captured.uid == candidate.uid && captured.skill_id == candidate.skill_id
        })
    };
    if draws.len() >= hand_size
        && round.ai_use_cards.len() == enemy_count
        && draws
            .iter()
            .all(|card| valid_identity(card, &player_candidates))
        && round
            .ai_use_cards
            .iter()
            .all(|card| valid_identity(card, &ai_candidates))
    {
        determinism.enqueue_start_decks(
            round.ai_use_cards.clone(),
            draws.iter().take(hand_size).cloned().collect(),
        );
        determinism.enqueue_card_draws(draws);
    }
    determinism
}

#[cfg(all(test, feature = "private-fixtures"))]
pub(crate) fn init_test_config() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json");
    config::init(
        data.to_str()
            .expect("workspace game-data path must be valid UTF-8"),
    )
    .expect("test game data must load");
}

pub fn preview_output_text(
    generated: &serde_json::Value,
    original: &serde_json::Value,
    original_text: String,
) -> anyhow::Result<String> {
    // if the generated object matches, reuse capture formatting for byte-stable diffs.
    if generated == original {
        Ok(original_text)
    } else {
        Ok(serde_json::to_string_pretty(generated)?)
    }
}

pub fn comparable_json(mut value: serde_json::Value) -> serde_json::Value {
    normalize_live_json(&mut value);
    prune_empty_json(&mut value);
    value
}

/// Canonical values used for parity checks, independent of capture presentation.
pub fn canonical_comparison(
    generated: serde_json::Value,
    captured: serde_json::Value,
) -> (serde_json::Value, serde_json::Value) {
    (comparable_json(generated), comparable_json(captured))
}

/// Applies capture casing and enum conventions for generated preview files only.
pub fn render_json_with_capture_conventions(
    generated: &serde_json::Value,
    template: &serde_json::Value,
) -> serde_json::Value {
    match (generated, template) {
        (serde_json::Value::Object(source), serde_json::Value::Object(shape)) => {
            let mut out = serde_json::Map::new();
            for (source_key, source_value) in source {
                let convention = shape
                    .iter()
                    .find(|(shape_key, _)| shape_key.eq_ignore_ascii_case(source_key));
                if let Some((shape_key, shape_value)) = convention {
                    out.insert(
                        shape_key.clone(),
                        render_json_with_capture_conventions(source_value, shape_value),
                    );
                } else {
                    out.insert(source_key.clone(), source_value.clone());
                }
            }
            serde_json::Value::Object(out)
        }
        (serde_json::Value::Array(source), serde_json::Value::Array(shape))
            if source.len() == shape.len()
                && source
                    .iter()
                    .zip(shape)
                    .all(|(source, shape)| array_elements_align(source, shape)) =>
        {
            serde_json::Value::Array(
                source
                    .iter()
                    .zip(shape)
                    .map(|(value, shape_value)| {
                        render_json_with_capture_conventions(value, shape_value)
                    })
                    .collect(),
            )
        }
        (serde_json::Value::Number(number), serde_json::Value::String(raw))
            if raw.parse::<i64>().is_ok() =>
        {
            serde_json::Value::String(number.to_string())
        }
        (serde_json::Value::Number(number), serde_json::Value::String(raw)) => {
            enum_name(number, raw)
                .map(|value| serde_json::Value::String(value.to_owned()))
                .unwrap_or_else(|| generated.clone())
        }
        _ => generated.clone(),
    }
}

fn array_elements_align(source: &serde_json::Value, shape: &serde_json::Value) -> bool {
    let (serde_json::Value::Object(source), serde_json::Value::Object(shape)) = (source, shape)
    else {
        return true;
    };
    let mut found_identity = false;
    for key in [
        "actType",
        "actId",
        "effectType",
        "targetId",
        "buffId",
        "uid",
        "skillId",
        "powerId",
    ] {
        let (Some(source), Some(shape)) = (source.get(key), shape.get(key)) else {
            continue;
        };
        found_identity = true;
        if render_json_with_capture_conventions(source, shape) != *shape {
            return false;
        }
    }
    found_identity
}

pub fn first_diff_path(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: &str,
) -> Option<String> {
    if left == right {
        return None;
    }

    match (left, right) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let mut keys: Vec<_> = left.keys().chain(right.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let next = format!("{path}/{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(path) = first_diff_path(left, right, &next) {
                            return Some(path);
                        }
                    }
                    _ => return Some(next),
                }
            }
            Some(path.to_owned())
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!("{path}.len"));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                if let Some(path) = first_diff_path(left, right, &format!("{path}[{index}]")) {
                    return Some(path);
                }
            }
            Some(path.to_owned())
        }
        _ => Some(path.to_owned()),
    }
}

pub fn value_at_diff_path<'a>(
    value: &'a serde_json::Value,
    path: &str,
    root: &str,
) -> Option<&'a serde_json::Value> {
    let mut value = value;
    let mut remaining = path.strip_prefix(root)?;
    while !remaining.is_empty() {
        if remaining == ".len" {
            return None;
        }
        if let Some(rest) = remaining.strip_prefix('/') {
            let end = rest.find(['/', '[', '.']).unwrap_or(rest.len());
            value = value.get(&rest[..end])?;
            remaining = &rest[end..];
            continue;
        }
        if let Some(rest) = remaining.strip_prefix('[') {
            let end = rest.find(']')?;
            value = value.get(rest[..end].parse::<usize>().ok()?)?;
            remaining = &rest[end + 1..];
            continue;
        }
        return None;
    }
    Some(value)
}

pub fn battle_id(reply_path: &Path) -> Option<i32> {
    let request_path = reply_path.with_file_name("StartDungeonRequest.json");
    let request: StartDungeonRequest =
        serde_json::from_str(&fs::read_to_string(request_path).ok()?).ok()?;
    request.episode_id
}

pub fn tower_plan_id(reply_path: &Path) -> Option<i32> {
    let request_path = reply_path.with_file_name("StartTowerBattleRequest.json");
    let request: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(request_path).ok()?).ok()?;
    request
        .get("talentPlanId")?
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
}

pub fn battle_inputs(
    root: &Path,
    args: Vec<String>,
    file_name: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    if args.is_empty() {
        return preview_files(root, file_name);
    }

    args.into_iter()
        .map(|arg| {
            let path = root.join(&arg).join(file_name);
            if path.exists() {
                Ok(path)
            } else {
                Ok(PathBuf::from(arg))
            }
        })
        .collect()
}

pub fn begin_round_inputs(root: &Path, args: Vec<String>) -> anyhow::Result<Vec<PathBuf>> {
    let battle_dirs = if args.is_empty() {
        fs::read_dir(root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        args.into_iter()
            .map(|arg| {
                let path = root.join(&arg);
                if path.is_dir() {
                    path
                } else {
                    PathBuf::from(arg)
                }
            })
            .collect()
    };
    let mut files = Vec::new();
    for path in battle_dirs {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let path = entry?.path();
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(is_begin_round_reply)
                {
                    files.push(path);
                }
            }
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn is_begin_round_reply(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.starts_with("begin_round_") && !name.contains("request"))
        || name.starts_with("beginroundreply_")
}

fn preview_files(root: &Path, file_name: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path().join(file_name);
        if path.exists() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn enum_name<'a>(number: &serde_json::Number, raw: &'a str) -> Option<&'a str> {
    let value = number.as_i64()?;
    if sonettobuf::card_info::CardStatus::from_str_name(raw)
        .is_some_and(|status| i64::from(status as i32) == value)
    {
        return Some(raw);
    }
    match raw {
        "Normal" if value == 1 => Some("Normal"),
        "NONE" if value == 0 => Some("NONE"),
        "SKILL" if value == 1 => Some("SKILL"),
        "BUFF" if value == 2 => Some("BUFF"),
        "EFFECT" if value == 3 => Some("EFFECT"),
        "CHANGEHERO" if value == 4 => Some("CHANGEHERO"),
        "CHANGEWAVE" if value == 5 => Some("CHANGEWAVE"),
        "Skill" if value == 1 => Some("Skill"),
        "SkillEffect" if value == 2 => Some("SkillEffect"),
        "Buff" if value == 3 => Some("Buff"),
        "Additional" if value == 4 => Some("Additional"),
        "AbsorbHurt" if value == 5 => Some("AbsorbHurt"),
        "ShareHurt" if value == 6 => Some("ShareHurt"),
        "FakeSkill" if value == 7 => Some("FakeSkill"),
        _ => None,
    }
}

fn prune_empty_json(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|_, value| !prune_empty_json(value));
            map.is_empty()
        }
        serde_json::Value::Array(values) => {
            for value in values.iter_mut() {
                prune_empty_json(value);
            }
            values.is_empty()
        }
        serde_json::Value::Null => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightRound, FightTeam};

    fn card(uid: i64, skill_id: i32, temp_card: bool) -> CardInfo {
        CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            temp_card: Some(temp_card),
            ..Default::default()
        }
    }

    #[test]
    fn captured_opening_rng_keeps_raw_draw_order_and_excludes_temporary_cards() {
        let fight = Fight {
            battle_id: Some(77),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    skill_group1: vec![101, 102],
                    skill_group2: vec![103, 104],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    skill_group1: vec![201],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let normal = vec![
            card(10, 101, false),
            card(10, 103, false),
            card(10, 103, false),
            card(10, 101, false),
        ];
        let ai = vec![card(-1, 201, false)];
        let mut determinism = super::captured_opening_determinism(
            &fight,
            &FightRound {
                team_a_cards1: vec![
                    normal[0].clone(),
                    normal[1].clone(),
                    card(10, 999, true),
                    normal[2].clone(),
                    normal[3].clone(),
                ],
                ai_use_cards: ai.clone(),
                ..Default::default()
            },
        );

        assert_eq!(
            determinism.take_start_decks(),
            Some((ai, normal[..3].to_vec()))
        );
        assert_eq!(determinism.draw_cards(&normal, normal.len()), normal);
    }

    #[test]
    fn captured_opening_rng_rejects_an_invalid_seed_as_a_whole() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    skill_group1: vec![101],
                    skill_group2: vec![102],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    skill_group1: vec![201],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut determinism = super::captured_opening_determinism(
            &fight,
            &FightRound {
                team_a_cards1: vec![
                    card(10, 101, false),
                    card(10, 999, false),
                    card(10, 102, false),
                ],
                ai_use_cards: vec![card(-1, 201, false)],
                ..Default::default()
            },
        );

        assert_eq!(determinism.take_start_decks(), None);

        let mut determinism = super::captured_opening_determinism(
            &fight,
            &FightRound {
                team_a_cards1: vec![
                    card(10, 101, false),
                    card(10, 102, false),
                    card(10, 101, false),
                ],
                ai_use_cards: Vec::new(),
                ..Default::default()
            },
        );

        assert_eq!(determinism.take_start_decks(), None);
    }

    #[test]
    fn resolves_the_value_reported_by_a_diff_path() {
        let value = json!({"steps": [{"effects": [{"energy": 2}]}]});

        assert_eq!(
            super::value_at_diff_path(
                &value["steps"],
                "/round/fightStep[0]/effects[0]/energy",
                "/round/fightStep",
            ),
            Some(&json!(2))
        );
    }

    use super::{
        canonical_comparison, first_diff_path, preview_output_text,
        render_json_with_capture_conventions,
    };

    #[test]
    fn canonical_comparison_reports_capture_only_fields() {
        let generated = json!({ "fight": { "version": 7 } });
        let captured = json!({ "fight": { "version": 7, "captureOnly": 9 } });

        let (generated, captured) = canonical_comparison(generated, captured);

        assert_eq!(
            first_diff_path(&generated, &captured, ""),
            Some("/fight/captureOnly".to_owned())
        );
    }

    #[test]
    fn capture_conventions_change_representation_without_shaping_fields() {
        let generated = json!({
            "uid": 42,
            "summonedList": [],
            "cardType": 0,
            "actType": 3,
            "damageFromType": 2,
            "extraDefault": null
        });
        let template = json!({
            "uid": "42",
            "SummonedList": [],
            "cardType": "NONE",
            "actType": "EFFECT",
            "damageFromType": "SkillEffect",
            "captureOnly": 9
        });

        assert_eq!(
            render_json_with_capture_conventions(&generated, &template),
            json!({
                "uid": "42",
                "SummonedList": [],
                "cardType": "NONE",
                "actType": "EFFECT",
                "damageFromType": "SkillEffect",
                "extraDefault": null
            })
        );
    }

    #[test]
    fn does_not_hide_misaligned_array_elements() {
        let generated = json!([{
            "actType": 1,
            "actId": 200,
            "actEffect": [{ "fightStep": { "actId": 201 } }]
        }]);
        let template = json!([{
            "actType": "EFFECT",
            "actId": 0,
            "actEffect": [{ "effectType": 1 }]
        }]);

        assert_eq!(
            render_json_with_capture_conventions(&generated, &template),
            generated
        );
    }

    #[test]
    fn preserves_capture_text_when_generated_object_matches() {
        let value = json!({ "fight": {}, "round": {} });
        let text = "{\r\n  \"fight\": {},\r\n  \"round\": {}\r\n}".to_owned();

        assert_eq!(
            preview_output_text(&value, &value, text.clone()).unwrap(),
            text
        );
    }

    #[test]
    fn recognizes_both_capture_round_reply_names() {
        assert!(super::is_begin_round_reply("begin_round_2.json"));
        assert!(super::is_begin_round_reply("BeginRoundReply_2.json"));
        assert!(!super::is_begin_round_reply("BeginRoundRequest_2.json"));
        assert!(!super::is_begin_round_reply("begin_round_2_request.json"));
    }

    #[test]
    fn normalizes_protobuf_custom_data_type_names() {
        let mut value = json!({
            "customData": [
                { "type": "TowerCompose", "data": "{}" },
                { "type": "Atomic", "data": "{}" }
            ]
        });

        super::normalize_live_json(&mut value);

        assert_eq!(value["customData"][0]["type"], 8);
        assert_eq!(value["customData"][1]["type"], 9);
    }

    #[test]
    fn normalizes_all_protobuf_card_status_names() {
        let mut value = json!({ "status": "STATUS_PLAYSETGRAY" });

        super::normalize_live_json(&mut value);

        assert_eq!(value["status"], 1);
    }

    #[test]
    fn normalizes_numeric_heat_scale_maximums() {
        let mut value = json!({ "heatScale": { "max": "750", "value": "121" } });

        super::normalize_live_json(&mut value);

        assert_eq!(value["heatScale"]["max"], 750);
        assert_eq!(value["heatScale"]["value"], 121);
    }

    #[test]
    fn tower_plan_comes_from_the_client_request() {
        let directory =
            std::env::temp_dir().join(format!("enigma-tower-plan-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("StartTowerBattleRequest.json"),
            r#"{"talentPlanId":401}"#,
        )
        .unwrap();
        let reply = directory.join("StartTowerBattleReply.json");

        assert_eq!(super::tower_plan_id(&reply), Some(401));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
