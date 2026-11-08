// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turnback {
    #[serde(rename = "additionChapterIds")]
    pub addition_chapter_ids: String,
    #[serde(rename = "additionDurationDays")]
    pub addition_duration_days: i32,
    #[serde(rename = "additionRate")]
    pub addition_rate: i32,
    #[serde(rename = "additionType")]
    pub addition_type: String,
    #[serde(rename = "bindActivityId")]
    pub bind_activity_id: i32,
    #[serde(rename = "bonusList")]
    pub bonus_list: String,
    #[serde(rename = "bonusPointMaterial")]
    pub bonus_point_material: String,
    #[serde(rename = "buyBonus")]
    pub buy_bonus: String,
    #[serde(rename = "buyDoubleBonusPrice")]
    pub buy_double_bonus_price: String,
    #[serde(rename = "canBuyDoubleBonus")]
    pub can_buy_double_bonus: bool,
    pub condition: String,
    #[serde(rename = "durationDays")]
    pub duration_days: i32,
    #[serde(rename = "endStory")]
    pub end_story: i32,
    #[serde(rename = "endTime")]
    pub end_time: String,
    pub id: i32,
    #[serde(rename = "jumpId")]
    pub jump_id: i32,
    #[serde(rename = "monthCardAddedId")]
    pub month_card_added_id: i32,
    pub name: String,
    #[serde(rename = "offlineDays")]
    pub offline_days: i32,
    #[serde(rename = "onceBonus")]
    pub once_bonus: String,
    #[serde(rename = "onlineDurationDays")]
    pub online_duration_days: i32,
    #[serde(rename = "openDailyBonus")]
    pub open_daily_bonus: bool,
    pub priority: i32,
    #[serde(rename = "resourceReturnCondition")]
    pub resource_return_condition: String,
    #[serde(rename = "resourceReturnVitality")]
    pub resource_return_vitality: String,
    #[serde(rename = "startStory")]
    pub start_story: i32,
    #[serde(rename = "subModuleIds")]
    pub sub_module_ids: String,
    #[serde(rename = "taskBonusMailId")]
    pub task_bonus_mail_id: i32,
}
use std::collections::HashMap;

pub struct TurnbackTable {
    records: Vec<Turnback>,
    by_id: HashMap<i32, usize>,
}

impl TurnbackTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Turnback> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Turnback> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Turnback] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Turnback> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}