// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterTemplate {
    #[serde(rename = "addDmg")]
    pub add_dmg: i32,
    #[serde(rename = "addDmgGrow")]
    pub add_dmg_grow: i32,
    pub attack: i32,
    #[serde(rename = "attackGrow")]
    pub attack_grow: i32,
    pub cri: i32,
    #[serde(rename = "criDef")]
    pub cri_def: i32,
    #[serde(rename = "criDefGrow")]
    pub cri_def_grow: i32,
    #[serde(rename = "criDmg")]
    pub cri_dmg: i32,
    #[serde(rename = "criDmgGrow")]
    pub cri_dmg_grow: i32,
    #[serde(rename = "criGrow")]
    pub cri_grow: i32,
    pub defense: i32,
    #[serde(rename = "defenseGrow")]
    pub defense_grow: i32,
    #[serde(rename = "dropDmg")]
    pub drop_dmg: i32,
    #[serde(rename = "dropDmgGrow")]
    pub drop_dmg_grow: i32,
    pub life: i32,
    #[serde(rename = "lifeGrow")]
    pub life_grow: i32,
    pub mdefense: i32,
    #[serde(rename = "mdefenseGrow")]
    pub mdefense_grow: i32,
    #[serde(rename = "multiHp")]
    pub multi_hp: String,
    pub recri: i32,
    #[serde(rename = "recriGrow")]
    pub recri_grow: i32,
    pub technic: i32,
    #[serde(rename = "technicGrow")]
    pub technic_grow: i32,
    pub template: i32,
}
use std::collections::HashMap;

pub struct MonsterTemplateTable {
    records: Vec<MonsterTemplate>,
    by_id: HashMap<i32, usize>,
}

impl MonsterTemplateTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<MonsterTemplate> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.entry(record.template).or_insert(idx);
        }

        Ok(Self {
            records,
            by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&MonsterTemplate> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[MonsterTemplate] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, MonsterTemplate> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}