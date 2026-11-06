// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTargetTypeDefine {
    pub define: i32,
    #[serde(rename = "logicTarget")]
    pub logic_target: String,
}
pub struct SkillTargetTypeDefineTable {
    records: Vec<SkillTargetTypeDefine>,
}

impl SkillTargetTypeDefineTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<SkillTargetTypeDefine> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[SkillTargetTypeDefine] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, SkillTargetTypeDefine> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}