// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2ActiveSkill {
    #[serde(rename = "assembleCost")]
    pub assemble_cost: i32,
    #[serde(rename = "attributeTag")]
    pub attribute_tag: String,
    #[serde(rename = "battleTag")]
    pub battle_tag: String,
    pub career: String,
    #[serde(rename = "countParam")]
    pub count_param: String,
    #[serde(rename = "countTitle")]
    pub count_title: String,
    pub desc: String,
    #[serde(rename = "descSimply")]
    pub desc_simply: String,
    pub hero_trial: String,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "isHide")]
    pub is_hide: i32,
    #[serde(rename = "isOff")]
    pub is_off: i32,
    #[serde(rename = "keyWord")]
    pub key_word: String,
    pub name: String,
    #[serde(rename = "newDesc")]
    pub new_desc: String,
    #[serde(rename = "outUnlock")]
    pub out_unlock: String,
    #[serde(rename = "outUnlockDesc")]
    pub out_unlock_desc: String,
    #[serde(rename = "passiveSkillId")]
    pub passive_skill_id: String,
    pub rare: i32,
    #[serde(rename = "skillId")]
    pub skill_id: i32,
    #[serde(rename = "skillTypeName")]
    pub skill_type_name: i32,
    #[serde(rename = "sortId")]
    pub sort_id: i32,
    pub tag: i32,
    pub unlock: String,
    #[serde(rename = "updateAttri")]
    pub update_attri: String,
    #[serde(rename = "updateSkill")]
    pub update_skill: i32,
}
use std::collections::HashMap;

pub struct Rouge2ActiveSkillTable {
    records: Vec<Rouge2ActiveSkill>,
    by_id: HashMap<i32, usize>,
}

impl Rouge2ActiveSkillTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2ActiveSkill> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self {
            records,
            by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Rouge2ActiveSkill> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2ActiveSkill] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2ActiveSkill> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}