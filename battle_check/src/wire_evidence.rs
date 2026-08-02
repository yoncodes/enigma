use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use battle::engine::{
    entity::skill::split_ids,
    skill::buff_act::wire::{self, WirePhase},
};
use config::configs::GameDB;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MarkerKey {
    opcode: i32,
    type_name: String,
    phase: u8,
    effect_type: i32,
}

impl MarkerKey {
    fn new(opcode: i32, type_name: &str, phase: WirePhase, effect_type: i32) -> Self {
        Self {
            opcode,
            type_name: type_name.to_owned(),
            phase: phase_id(phase),
            effect_type,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Evidence {
    sources: BTreeMap<MarkerKey, BTreeSet<String>>,
}

impl Evidence {
    pub(crate) fn collect(db: &GameDB) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../battle_preview/fixtures/battles");
        let mut evidence = Self::default();
        for path in response_files(&root) {
            if path.parent().and_then(fight_version) != Some(7) {
                continue;
            }
            let Some(source) = path
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(mut value) = serde_json::from_str(&contents) else {
                continue;
            };
            if battle_preview::expand_compressed_fight_steps(&mut value).is_err() {
                continue;
            }
            evidence.inspect(&value, db, source);
        }
        evidence
    }

    pub(crate) fn source(
        &self,
        opcode: i32,
        type_name: &str,
        phase: WirePhase,
        effect_type: i32,
    ) -> Option<String> {
        self.sources
            .get(&MarkerKey::new(opcode, type_name, phase, effect_type))
            .map(|sources| sources.iter().cloned().collect::<Vec<_>>().join(","))
    }

    fn inspect(&mut self, value: &Value, db: &GameDB, source: &str) {
        match value {
            Value::Object(object) => {
                if let Some(Value::Array(effects)) = object.get("actEffect") {
                    self.inspect_effects(effects, db, source);
                }
                for child in object.values() {
                    self.inspect(child, db, source);
                }
            }
            Value::Array(values) => {
                for child in values {
                    self.inspect(child, db, source);
                }
            }
            _ => {}
        }
    }

    fn inspect_effects(&mut self, effects: &[Value], db: &GameDB, source: &str) {
        for (index, effect) in effects.iter().enumerate() {
            self.inspect_direct_marker(effect, db, source);
            let Some(phase) = effect_phase(effect) else {
                continue;
            };
            let Some(buff_id) = effect
                .get("buff")
                .and_then(|buff| buff.get("buffId"))
                .and_then(Value::as_i64)
                .and_then(|id| i32::try_from(id).ok())
            else {
                continue;
            };
            let Some(buff) = db.skill_buff.get(buff_id) else {
                continue;
            };

            let expected = buff
                .features
                .split('|')
                .filter_map(|feature| split_ids(feature).first().copied())
                .filter_map(|id| db.buff_act.get(id))
                .filter_map(|act| {
                    wire::find(act.id, &act.r#type).map(|definition| (act, definition))
                })
                .flat_map(|(act, definition)| {
                    definition
                        .markers(phase)
                        .iter()
                        .copied()
                        .map(move |effect_type| {
                            MarkerKey::new(act.id, &act.r#type, phase, effect_type)
                        })
                })
                .collect::<Vec<_>>();
            if expected.is_empty() || effects.len() < index + 1 + expected.len() {
                continue;
            }
            let actual = effects[index + 1..index + 1 + expected.len()]
                .iter()
                .filter_map(|effect| effect.get("effectType").and_then(Value::as_i64))
                .collect::<Vec<_>>();
            if actual.len() != expected.len()
                || !actual
                    .iter()
                    .zip(&expected)
                    .all(|(&actual, expected)| actual == i64::from(expected.effect_type))
            {
                continue;
            }
            for key in expected {
                self.sources
                    .entry(key)
                    .or_default()
                    .insert(source.to_owned());
            }
        }
    }

    fn inspect_direct_marker(&mut self, effect: &Value, db: &GameDB, source: &str) {
        let Some((buff, effect_type)) = effect
            .get("buff")
            .and_then(|buff| buff.get("buffId"))
            .and_then(Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
            .and_then(|id| db.skill_buff.get(id))
            .zip(
                effect
                    .get("effectType")
                    .and_then(Value::as_i64)
                    .and_then(|id| i32::try_from(id).ok()),
            )
        else {
            return;
        };
        let candidates = buff
            .features
            .split('|')
            .filter_map(|feature| split_ids(feature).first().copied())
            .filter_map(|id| db.buff_act.get(id))
            .filter_map(|act| wire::find(act.id, &act.r#type).map(|definition| (act, definition)))
            .flat_map(|(act, definition)| {
                [WirePhase::Add, WirePhase::Static, WirePhase::Refresh]
                    .into_iter()
                    .filter(move |phase| definition.markers(*phase).contains(&effect_type))
                    .map(move |phase| MarkerKey::new(act.id, &act.r#type, phase, effect_type))
            })
            .collect::<Vec<_>>();
        if let [key] = candidates.as_slice() {
            self.sources
                .entry(key.clone())
                .or_default()
                .insert(source.to_owned());
        }
    }
}

fn fight_version(battle_dir: &Path) -> Option<i32> {
    let path = ["StartDungeonReply.json", "StartTowerBattleReply.json"]
        .into_iter()
        .map(|name| battle_dir.join(name))
        .find(|path| path.exists())?;
    let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    value
        .get("startDungeonReply")
        .unwrap_or(&value)
        .get("fight")?
        .get("version")?
        .as_i64()
        .and_then(|version| i32::try_from(version).ok())
}

fn effect_phase(effect: &Value) -> Option<WirePhase> {
    match effect.get("effectType").and_then(Value::as_i64) {
        Some(5) => Some(WirePhase::Add),
        Some(7) => Some(WirePhase::Refresh),
        _ => None,
    }
}

fn phase_id(phase: WirePhase) -> u8 {
    match phase {
        WirePhase::Add => 0,
        WirePhase::Static => 1,
        WirePhase::Refresh => 2,
    }
}

fn response_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(battles) = fs::read_dir(root) else {
        return files;
    };
    for battle in battles.flatten() {
        let Ok(entries) = fs::read_dir(battle.path()) else {
            continue;
        };
        files.extend(entries.flatten().filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_ascii_lowercase();
            (path
                .extension()
                .is_some_and(|extension| extension == "json")
                && !name.contains("request")
                && (name.contains("startdungeonreply")
                    || name.contains("starttowerbattlereply")
                    || name.contains("beginroundreply")
                    || name.starts_with("begin_round_")))
            .then_some(path)
        }));
    }
    files
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn tower_wrapper_supplies_the_embedded_fight_version() {
        let root =
            std::env::temp_dir().join(format!("enigma-wire-evidence-{}", std::process::id()));
        let path = root.join("tower");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("StartTowerBattleReply.json"),
            r#"{"startDungeonReply":{"fight":{"version":7}}}"#,
        )
        .unwrap();

        assert_eq!(fight_version(&path), Some(7));
        assert_eq!(
            response_files(&root),
            vec![path.join("StartTowerBattleReply.json")]
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, feature = "private-fixtures"))]
mod tests {
    use super::*;
    use sonettobuf::effect_type_enum::EffectType;

    #[test]
    fn derives_marker_evidence_from_captures() {
        crate::init_config().unwrap();
        let evidence = Evidence::collect(config::configs::get());

        assert!(
            evidence
                .source(
                    1048,
                    "Radiance",
                    WirePhase::Add,
                    EffectType::Radiance as i32,
                )
                .is_none_or(|sources| !sources.split(',').any(|source| source == "battle6"))
        );
        let fixtures =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../battle_preview/fixtures/battles");
        assert_eq!(fight_version(&fixtures.join("battle75")), Some(7));
        assert!(
            evidence
                .source(
                    112,
                    "AttrOnlyCalDamageBeAttacked",
                    WirePhase::Static,
                    EffectType::Attr as i32,
                )
                .is_none()
        );
        for (phase, effect_type) in [
            (WirePhase::Add, EffectType::Redorbluecount as i32),
            (WirePhase::Refresh, EffectType::Redorbluecountchange as i32),
        ] {
            assert_eq!(
                evidence.source(897, "RedOrBlueCount", phase, effect_type),
                Some("battle75".to_owned())
            );
        }
    }
}
