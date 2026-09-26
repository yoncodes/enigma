// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummonPool {
    pub advertising: String,
    #[serde(rename = "awardTime")]
    pub award_time: String,
    pub banner: String,
    #[serde(rename = "bannerFlag")]
    pub banner_flag: i32,
    #[serde(rename = "bannerLineName")]
    pub banner_line_name: String,
    #[serde(rename = "changeWeight")]
    pub change_weight: String,
    #[serde(rename = "characterDetail")]
    pub character_detail: String,
    pub cost1: String,
    pub cost10: String,
    #[serde(rename = "customClz")]
    pub custom_clz: String,
    #[serde(rename = "dailyFreeSummon10Count")]
    pub daily_free_summon10_count: i32,
    pub desc: String,
    #[serde(rename = "discountCost10")]
    pub discount_cost10: String,
    #[serde(rename = "discountTime10")]
    pub discount_time10: i32,
    #[serde(rename = "doubleSsrUpRates")]
    pub double_ssr_up_rates: String,
    #[serde(rename = "free10MaxUseCount")]
    pub free10_max_use_count: i32,
    #[serde(rename = "freeTagStyle")]
    pub free_tag_style: String,
    #[serde(rename = "guaranteeSRParam")]
    pub guarantee_srparam: String,
    #[serde(rename = "historyShowType")]
    pub history_show_type: i32,
    pub id: i32,
    #[serde(rename = "infallibleItemId")]
    pub infallible_item_id: i32,
    #[serde(rename = "infallibleItemMaxUseCount")]
    pub infallible_item_max_use_count: i32,
    #[serde(rename = "infallibleMailIds")]
    pub infallible_mail_ids: String,
    #[serde(rename = "initWeight")]
    pub init_weight: String,
    #[serde(rename = "jumpGroupId")]
    pub jump_group_id: i32,
    #[serde(rename = "mailIds")]
    pub mail_ids: String,
    #[serde(rename = "maskColor")]
    pub mask_color: String,
    #[serde(rename = "nameCn")]
    pub name_cn: String,
    #[serde(rename = "nameEn")]
    pub name_en: String,
    #[serde(rename = "nameUnderlayColor")]
    pub name_underlay_color: String,
    #[serde(rename = "ornamentName")]
    pub ornament_name: String,
    pub param: String,
    pub param2: String,
    #[serde(rename = "poolDetail")]
    pub pool_detail: i32,
    #[serde(rename = "prefabPath")]
    pub prefab_path: String,
    #[serde(rename = "priorCost1")]
    pub prior_cost1: String,
    #[serde(rename = "priorCost10")]
    pub prior_cost10: String,
    pub priority: i32,
    #[serde(rename = "progressChooseGroupId")]
    pub progress_choose_group_id: String,
    #[serde(rename = "progressRewardClass")]
    pub progress_reward_class: String,
    #[serde(rename = "progressRewardPrefab")]
    pub progress_reward_prefab: String,
    #[serde(rename = "progressRewards")]
    pub progress_rewards: String,
    #[serde(rename = "spinePrefab")]
    pub spine_prefab: String,
    #[serde(rename = "subType")]
    pub sub_type: i32,
    #[serde(rename = "ticketId")]
    pub ticket_id: i32,
    #[serde(rename = "totalFreeCount")]
    pub total_free_count: i32,
    #[serde(rename = "totalPosibility")]
    pub total_posibility: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
    #[serde(rename = "unchosenMailId")]
    pub unchosen_mail_id: i32,
    #[serde(rename = "upWeight")]
    pub up_weight: String,
}
use std::collections::HashMap;

pub struct SummonPoolTable {
    records: Vec<SummonPool>,
    by_id: HashMap<i32, usize>,
    by_group: HashMap<i32, Vec<usize>>,
}

impl SummonPoolTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<SummonPool> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());
        let mut by_group: HashMap<i32, Vec<usize>> = HashMap::new();

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
            by_group.entry(record.jump_group_id).or_default().push(idx);
        }

        Ok(Self {
            records,
            by_id,
            by_group,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&SummonPool> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    pub fn by_group(&self, group_id: i32) -> impl Iterator<Item = &'_ SummonPool> + '_ {
        self.by_group
            .get(&group_id)
            .into_iter()
            .flat_map(|idxs| idxs.iter())
            .map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[SummonPool] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, SummonPool> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
