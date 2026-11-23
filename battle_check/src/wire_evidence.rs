use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use battle::tooling::scan::buff_act::{
    registry as buff_act_registry,
    wire::{self, WirePhase},
};
use config::configs::GameDB;
use serde_json::Value;

mod identity;
#[cfg(test)]
use identity::FightIdentity;
use identity::IdentityIndex;

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
    carrier_sources: BTreeMap<(i32, String, i32), BTreeSet<String>>,
}

impl Evidence {
    pub(crate) fn collect(db: &GameDB, capture_roots: &[PathBuf]) -> Self {
        let default_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../battle_preview/fixtures/battles");
        let roots = if capture_roots.is_empty() {
            std::slice::from_ref(&default_root)
        } else {
            capture_roots
        };
        let mut evidence = Self::default();
        let mut warned_ambiguous = BTreeSet::new();
        for root in roots {
            let mut directories = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
            for path in response_files(root) {
                if let Some(directory) = path.parent() {
                    directories
                        .entry(directory.to_owned())
                        .or_default()
                        .push(path);
                }
            }
            for (battle_dir, paths) in directories {
                let identities = IdentityIndex::collect(&battle_dir);
                for path in paths {
                    let identity = identities
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|index| index.response(&path));
                    let (episode_id, battle_id) = match identity {
                        Ok(Some((7, episode_id, battle_id))) => (episode_id, battle_id),
                        Ok(_) => continue,
                        Err(error) => {
                            if warned_ambiguous.insert((battle_dir.clone(), error.clone())) {
                                eprintln!(
                                    "WARN skipping capture replies: {error}; first affected reply: {}",
                                    path.display()
                                );
                            }
                            continue;
                        }
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
                    evidence.inspect(
                        &value,
                        db,
                        &format!("episode {episode_id} / battle {battle_id}"),
                    );
                }
            }
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

    pub(crate) fn carrier_sources(
        &self,
        opcode: i32,
        type_name: &str,
        buff_id: i32,
    ) -> Vec<String> {
        self.carrier_sources
            .get(&(opcode, type_name.to_owned(), buff_id))
            .map_or_else(Vec::new, |sources| sources.iter().cloned().collect())
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
                .filter_map(|raw| buff_act_registry::resolve_feature(Some(db), raw))
                .filter(|feature| !feature.is_malformed())
                .filter_map(|feature| feature.act_id)
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
                    .entry(key.clone())
                    .or_default()
                    .insert(source.to_owned());
                self.carrier_sources
                    .entry((key.opcode, key.type_name, buff_id))
                    .or_default()
                    .insert(source.to_owned());
            }
        }
    }

    fn inspect_direct_marker(&mut self, effect: &Value, db: &GameDB, source: &str) {
        let Some(effect_type) = effect
            .get("effectType")
            .and_then(Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            return;
        };
        let (definitions, buff_id) = if let Some(buff) = effect
            .get("buff")
            .and_then(|buff| buff.get("buffId"))
            .and_then(Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
            .and_then(|id| db.skill_buff.get(id))
        {
            (
                buff.features
                    .split('|')
                    .filter_map(|raw| buff_act_registry::resolve_feature(Some(db), raw))
                    .filter(|feature| !feature.is_malformed())
                    .filter_map(|feature| feature.act_id)
                    .filter_map(|id| db.buff_act.get(id))
                    .collect::<Vec<_>>(),
                buff.id,
            )
        } else {
            let Some((act, carrier)) = effect
                .get("buffActId")
                .and_then(Value::as_i64)
                .and_then(|id| i32::try_from(id).ok())
                .and_then(|id| db.buff_act.get(id))
                .zip(
                    effect
                        .get("effectNum")
                        .and_then(Value::as_i64)
                        .and_then(|id| i32::try_from(id).ok())
                        .and_then(|id| db.skill_buff.get(id)),
                )
            else {
                return;
            };
            if !carrier
                .features
                .split('|')
                .filter_map(|raw| buff_act_registry::resolve_feature(Some(db), raw))
                .any(|feature| !feature.is_malformed() && feature.act_id == Some(act.id))
            {
                return;
            }
            (vec![act], carrier.id)
        };
        let candidates = definitions
            .into_iter()
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
            self.carrier_sources
                .entry((key.opcode, key.type_name.clone(), buff_id))
                .or_default()
                .insert(source.to_owned());
        }
    }
}

#[cfg(test)]
fn fight_identity(battle_dir: &Path) -> Result<Option<(i32, i64, i64)>, ()> {
    let identities = fs::read_dir(battle_dir)
        .map_err(|_| ())?
        .flatten()
        .filter(|entry| is_setup_name(&entry.file_name().to_string_lossy()))
        .filter_map(|entry| {
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return None;
            }
            let value: Value =
                serde_json::from_str(&fs::read_to_string(entry.path()).ok()?).ok()?;
            let fight = find_fight(&value)?;
            Some((
                i32::try_from(fight.get("version")?.as_i64()?).ok()?,
                fight.get("episodeId")?.as_i64()?,
                fight.get("battleId")?.as_i64()?,
            ))
        })
        .collect::<BTreeSet<_>>();
    match identities.len() {
        0 => Ok(None),
        1 => Ok(identities.into_iter().next()),
        _ => Err(()),
    }
}

#[cfg(test)]
fn response_identity(response: &Path) -> Result<Option<FightIdentity>, ()> {
    let directory = response.parent().ok_or(())?;
    IdentityIndex::collect(directory)
        .map_err(|_| ())?
        .response(response)
        .map_err(|_| ())
}

fn is_setup_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".json")
        && (name.contains("startdungeonreply")
            || (name.contains("start") && name.contains("battle") && name.contains("reply")))
}

fn find_fight(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    match value {
        Value::Object(object) => object
            .get("fight")
            .and_then(Value::as_object)
            .filter(|fight| fight.contains_key("version"))
            .or_else(|| object.values().find_map(find_fight)),
        Value::Array(values) => values.iter().find_map(find_fight),
        _ => None,
    }
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
    capture_json_files(root)
        .into_iter()
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            let name = name.to_ascii_lowercase();
            !name.contains("request")
                && (is_setup_name(&name)
                    || name.contains("beginroundreply")
                    || name.starts_with("begin_round_")
                    || name.contains("autoroundreply")
                    || name.starts_with("auto_round_"))
        })
        .collect()
}

pub(crate) fn capture_json_files(root: &Path) -> Vec<PathBuf> {
    let Ok(root_metadata) = fs::symlink_metadata(root) else {
        return Vec::new();
    };
    if !root_metadata.is_dir() || is_link_or_reparse(&root_metadata) {
        return Vec::new();
    }
    let Ok(root) = root.canonicalize() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut decoded_files = Vec::new();
    let mut visited = BTreeSet::new();
    let root_is_decoded = root.components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("decoded")
    });
    let mut found_decoded = root_is_decoded;
    let mut directories = vec![(root.clone(), root_is_decoded)];
    while let Some((directory, inside_decoded)) = directories.pop() {
        let Ok(canonical) = directory.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(&root) || !visited.insert(canonical.clone()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&canonical) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            let named_decoded = entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("decoded");
            if is_link_or_reparse(&metadata) {
                found_decoded |= named_decoded;
                continue;
            }
            if metadata.is_dir() {
                let inside_decoded = inside_decoded || named_decoded;
                found_decoded |= inside_decoded;
                directories.push((path, inside_decoded));
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                if inside_decoded {
                    decoded_files.push(path.clone());
                }
                files.push(path);
            }
        }
    }
    // Broad version roots also contain static dumps; decoded subtrees are the packet source.
    let mut files = if found_decoded { decoded_files } else { files };
    files.sort();
    files
}

pub(crate) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests;
