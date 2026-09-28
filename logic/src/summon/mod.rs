use crate::{error::AppError, reward};
use database::{
    db::{
        game::{guides, summon},
        user::account,
    },
    models::game::{currencies::UserCurrencyModel, heros::UserHeroModel, items::UserItemModel},
};
use rand::{Rng, prelude::IndexedRandom};
use sonettobuf::{
    ChooseEnhancedPoolHeroReply, ChooseMultiUpHeroReply, EndActivityPush, GetSummonInfoReply,
    GetSummonProgressRewardsReply, GuideInfo, PopUpRecommendWindowReply, SummonQueryTokenReply,
    SummonReply, SummonResult,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
mod commands;
mod parse;
mod pool;

pub use commands::SummonCompletion;
use parse::{choose_weighted, parse_ids, parse_up_heroes, parse_weighted};
pub(crate) use pool::build_gacha_pool;
use pool::{GachaResult, GachaRules, GachaState, SummonType};

#[derive(Clone, Copy, Debug)]
pub struct SummonManager {
    player_id: i64,
}

impl SummonManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(&self, db: &SqlitePool) -> Result<GetSummonInfoReply, AppError> {
        let visible = visible_pools_at(common::time::ServerTime::now_sec_i32());
        summon::sync_summon_pools(db, self.player_id, &visible).await?;
        commands::summon_info(db, self.player_id, &visible).await
    }

    pub async fn progress_rewards(
        &self,
        db: &SqlitePool,
        pool_id: i32,
    ) -> Result<(GetSummonProgressRewardsReply, Vec<u32>), AppError> {
        commands::progress_rewards(db, self.player_id, pool_id).await
    }

    pub async fn pop_up_recommend_window(
        &self,
        db: &SqlitePool,
        pool_id: i32,
        order_id: i32,
    ) -> Result<PopUpRecommendWindowReply, AppError> {
        commands::pop_up_recommend_window(db, self.player_id, pool_id, order_id).await
    }

    pub async fn query_token(
        &self,
        db: &SqlitePool,
    ) -> Result<(SummonQueryTokenReply, EndActivityPush), AppError> {
        commands::query_token(db, self.player_id).await
    }

    pub async fn summon(
        &self,
        db: &SqlitePool,
        pool_id: i32,
        guide_id: Option<i32>,
        step_id: Option<i32>,
        count: i32,
    ) -> Result<SummonCompletion, AppError> {
        commands::summon(db, self.player_id, pool_id, guide_id, step_id, count).await
    }

    pub async fn choose_enhanced_pool_hero(
        &self,
        db: &SqlitePool,
        pool_id: i32,
        hero_id: i32,
    ) -> Result<ChooseEnhancedPoolHeroReply, AppError> {
        commands::choose_enhanced_pool_hero(db, self.player_id, pool_id, hero_id).await
    }

    pub async fn choose_multi_up_hero(
        &self,
        db: &SqlitePool,
        pool_id: i32,
        hero_ids: Vec<i32>,
    ) -> Result<ChooseMultiUpHeroReply, AppError> {
        commands::choose_multi_up_hero(db, self.player_id, pool_id, hero_ids).await
    }
}

fn visible_pools_at(now_sec: i32) -> Vec<database::models::game::summon::SummonPoolWindow> {
    #[cfg(test)]
    crate::init_test_server_config();
    let tables = config::configs::get();
    let active = scheduled_pools()
        .into_iter()
        .filter(|pool| pool.online_time <= now_sec && now_sec <= pool.offline_time)
        .collect::<Vec<_>>();
    let version = active
        .iter()
        .max_by_key(|pool| pool.online_time)
        .and_then(|pool| tables.summon_pool.get(pool.pool_id))
        .and_then(|pool| summon_version(&pool.prefab_path));

    let mut visible = active
        .into_iter()
        .filter(|visible| {
            tables.summon_pool.get(visible.pool_id).is_some_and(|pool| {
                pool.r#type == 1 || summon_version(&pool.prefab_path) == version
            })
        })
        .map(|pool| (pool.pool_id, pool))
        .collect::<BTreeMap<_, _>>();
    for pool_id in &common::config().summon.permanent_pool_ids {
        if let Some(pool) = tables.summon_pool.get(*pool_id) {
            visible.insert(
                pool.id,
                database::models::game::summon::SummonPoolWindow {
                    pool_id: pool.id,
                    online_time: 0,
                    offline_time: i32::MAX,
                    discount_time: pool.discount_time10,
                },
            );
        }
    }
    visible.into_values().collect()
}

fn scheduled_pools() -> Vec<database::models::game::summon::SummonPoolWindow> {
    use database::models::game::summon::SummonPoolWindow;

    let tables = config::configs::get();
    let mut by_pool = BTreeMap::new();
    for store in tables
        .store_recommend
        .iter()
        .filter(|store| store.is_offline == 0)
    {
        let Some(pool_id) = parse_pool_relation(&store.relations) else {
            continue;
        };
        let Some(pool) = tables.summon_pool.get(pool_id) else {
            continue;
        };
        let (online_time, offline_time) = if pool.r#type == 1 {
            (0, i32::MAX)
        } else {
            let (Some(online), Some(offline)) = (
                parse_ts_seconds(&store.online_time),
                parse_ts_seconds(&store.offline_time),
            ) else {
                continue;
            };
            (online, offline)
        };
        by_pool.entry(pool_id).or_insert(SummonPoolWindow {
            pool_id,
            online_time,
            offline_time,
            discount_time: pool.discount_time10,
        });
    }
    by_pool.into_values().collect()
}

#[cfg(test)]
fn visible_summon_pool_ids_at(now_sec: i32) -> Vec<i32> {
    visible_pools_at(now_sec)
        .into_iter()
        .map(|pool| pool.pool_id)
        .collect()
}

fn summon_version(prefab_path: &str) -> Option<&str> {
    prefab_path
        .split(['/', '\\'])
        .next()
        .filter(|version| version.starts_with("version_"))
}

fn parse_pool_relation(relations: &str) -> Option<i32> {
    relations
        .split('|')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("1#")?.parse().ok())
}

fn parse_ts_seconds(value: &str) -> Option<i32> {
    chrono::NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|time| time.and_utc().timestamp() as i32)
}

#[cfg(test)]
mod test;
