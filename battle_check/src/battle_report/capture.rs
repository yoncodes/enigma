use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

#[derive(Default)]
pub(crate) struct Evidence {
    skills: BTreeSet<i32>,
    buffs: BTreeSet<i32>,
    episodes: BTreeSet<i32>,
}

impl Evidence {
    pub(crate) fn collect(roots: &[PathBuf]) -> Self {
        let mut evidence = Self::default();
        let default_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../battle_preview/fixtures/battles");
        let roots = if roots.is_empty() {
            std::slice::from_ref(&default_root)
        } else {
            roots
        };
        for root in roots {
            for path in crate::wire_evidence::bounded_json_files(root) {
                let Ok(contents) = fs::read_to_string(path) else {
                    continue;
                };
                let Ok(mut value) = serde_json::from_str(&contents) else {
                    continue;
                };
                let _ = battle_preview::expand_compressed_fight_steps(&mut value);
                evidence.inspect(&value);
            }
        }
        evidence
    }

    pub(crate) fn skill(&self, id: i32) -> bool {
        self.skills.contains(&id)
    }

    pub(crate) fn buff(&self, id: i32) -> bool {
        self.buffs.contains(&id)
    }

    pub(crate) fn episode(&self, id: i32) -> bool {
        self.episodes.contains(&id)
    }

    fn inspect(&mut self, value: &Value) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    match key.as_str() {
                        "skillId" => insert_number(child, &mut self.skills),
                        "skillGroup1" | "skillGroup2" | "passiveSkill" | "exSkill" => {
                            insert_numbers(child, &mut self.skills)
                        }
                        "buffId" => insert_number(child, &mut self.buffs),
                        "episodeId" => insert_number(child, &mut self.episodes),
                        _ => {}
                    }
                    self.inspect(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    self.inspect(child);
                }
            }
            _ => {}
        }
    }
}

fn insert_numbers(value: &Value, output: &mut BTreeSet<i32>) {
    match value {
        Value::Array(values) => {
            for value in values {
                insert_number(value, output);
            }
        }
        _ => insert_number(value, output),
    }
}

fn insert_number(value: &Value, output: &mut BTreeSet<i32>) {
    if let Some(value) = value.as_i64().and_then(|value| i32::try_from(value).ok())
        && value > 0
    {
        output.insert(value);
    }
}
