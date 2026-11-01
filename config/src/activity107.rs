//! Store entries extracted from the user's matching client configuration.
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Goods {
    pub activity_id: i32,
    pub id: i32,
    pub product: String,
    pub max_buy_count: i32,
    pub cost: String,
    pub is_online: bool,
    pub pre_goods_id: i32,
}

static GOODS: OnceLock<Result<Vec<Goods>, String>> = OnceLock::new();

pub fn goods() -> anyhow::Result<&'static [Goods]> {
    match GOODS.get_or_init(|| {
        let rows: Vec<Goods> = serde_json::from_str(include_str!("../data/activity107_goods.json"))
            .map_err(|error| error.to_string())?;
        let mut keys = HashSet::new();
        for row in &rows {
            if row.activity_id <= 0 || row.id <= 0 || row.max_buy_count < 0 || !keys.insert((row.activity_id,row.id)) {
                return Err("invalid or duplicate activity-store entry".to_owned());
            }
        }
        Ok(rows)
    }) {
        Ok(rows) => Ok(rows),
        Err(error) => Err(anyhow::anyhow!("invalid activity-store configuration: {error}")),
    }
}

pub fn find(activity_id: i32, goods_id: i32) -> anyhow::Result<Option<&'static Goods>> {
    Ok(goods()?.iter().find(|row| row.activity_id == activity_id && row.id == goods_id))
}
