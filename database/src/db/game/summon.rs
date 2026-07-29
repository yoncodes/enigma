use crate::models::game::summon::*;
use anyhow::Result;
use chrono::{NaiveDateTime, TimeZone, Utc};
use common::time::ServerTime;
use sonettobuf::SummonResult;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{BTreeMap, HashMap, HashSet};

pub async fn get_summon_stats(pool: &SqlitePool, user_id: i64) -> Result<UserSummonStats> {
    let stats =
        sqlx::query_as::<_, UserSummonStats>("SELECT * FROM user_summon_stats WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    Ok(stats.unwrap_or(UserSummonStats {
        user_id,
        free_equip_summon: false,
        is_show_new_summon: false,
        new_summon_count: 0,
        total_summon_count: 0,
    }))
}

pub async fn get_gacha_state(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
) -> Result<Option<(u32, bool)>> {
    let row: Option<(i64, bool)> = sqlx::query_as(
        "SELECT pity_6, up_guaranteed
         FROM user_gacha_state
         WHERE user_id = ? AND pool_id = ?",
    )
    .bind(user_id)
    .bind(pool_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(pity, guaranteed)| (pity as u32, guaranteed)))
}

pub async fn save_gacha_state(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    pity_6: u32,
    up_guaranteed: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_gacha_state
             (user_id, pool_id, pity_6, up_guaranteed, last_pull_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, pool_id) DO UPDATE SET
             pity_6 = excluded.pity_6,
             up_guaranteed = excluded.up_guaranteed,
             last_pull_at = excluded.last_pull_at",
    )
    .bind(user_id)
    .bind(pool_id)
    .bind(pity_6)
    .bind(up_guaranteed)
    .bind(ServerTime::now_ms())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn save_gacha_state_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    pool_id: i32,
    expected: Option<(u32, bool)>,
    pity_6: u32,
    up_guaranteed: bool,
) -> Result<bool> {
    let result = if let Some((expected_pity, expected_guaranteed)) = expected {
        sqlx::query(
            "UPDATE user_gacha_state
             SET pity_6 = ?, up_guaranteed = ?, last_pull_at = ?
             WHERE user_id = ? AND pool_id = ?
               AND pity_6 = ? AND up_guaranteed = ?",
        )
        .bind(pity_6)
        .bind(up_guaranteed)
        .bind(ServerTime::now_ms())
        .bind(user_id)
        .bind(pool_id)
        .bind(expected_pity)
        .bind(expected_guaranteed)
        .execute(&mut **tx)
        .await?
    } else {
        sqlx::query(
            "INSERT OR IGNORE INTO user_gacha_state
                 (user_id, pool_id, pity_6, up_guaranteed, last_pull_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(pool_id)
        .bind(pity_6)
        .bind(up_guaranteed)
        .bind(ServerTime::now_ms())
        .execute(&mut **tx)
        .await?
    };
    Ok(result.rows_affected() == 1)
}

pub async fn get_summon_pool_infos(pool: &SqlitePool, user_id: i64) -> Result<Vec<SummonPoolInfo>> {
    let visible_pools = visible_pools();
    if visible_pools.is_empty() {
        return Ok(vec![]);
    }
    let visible_pool_ids = visible_pools
        .iter()
        .map(|pool| pool.pool_id)
        .collect::<HashSet<_>>();

    // Batch-load all user pool rows in one query
    let user_pools: HashMap<i32, UserSummonPool> =
        sqlx::query_as::<_, UserSummonPool>("SELECT * FROM user_summon_pools WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .filter(|p| visible_pool_ids.contains(&p.pool_id))
            .map(|p| (p.pool_id, p))
            .collect();

    // Batch-load lucky bags
    let lucky_bags = load_all_lucky_bags(pool, user_id).await?;

    // Batch-load sp pool base rows
    let sp_pools = load_all_sp_pools(pool, user_id).await?;

    // Batch-load pop-up infos
    let all_pop_up_infos = load_all_pop_up_infos(pool, user_id).await?;

    let now = ServerTime::now_ms();

    let result = visible_pools
        .into_iter()
        .map(|visible| {
            let pool_data = user_pools.get(&visible.pool_id).cloned().unwrap_or({
                UserSummonPool {
                    id: 0,
                    user_id,
                    pool_id: visible.pool_id,
                    online_time: visible.online_time,
                    offline_time: visible.offline_time,
                    have_free: false,
                    used_free_count: 0,
                    discount_time: visible.discount_time,
                    can_get_guarantee_sr_count: 0,
                    guarantee_sr_countdown: 0,
                    summon_count: 0,
                    have_free10_count: 0,
                    not_ssr_count: 0,
                    total_free10_use_count: 0,
                    created_at: now,
                    updated_at: now,
                }
            });

            SummonPoolInfo {
                lucky_bag: lucky_bags.get(&visible.pool_id).cloned(),
                sp_pool: sp_pools.get(&visible.pool_id).cloned(),
                pop_up_infos: all_pop_up_infos
                    .get(&visible.pool_id)
                    .cloned()
                    .unwrap_or_default(),
                pool: pool_data,
            }
        })
        .collect();

    Ok(result)
}

#[derive(Clone)]
struct VisibleSummonPool {
    pool_id: i32,
    online_time: i32,
    offline_time: i32,
    discount_time: i32,
}

pub async fn sync_visible_pools(pool: &SqlitePool, user_id: i64) -> Result<()> {
    let visible = visible_pools();
    let now = ServerTime::now_ms();
    for visible_pool in &visible {
        sqlx::query(
            r#"
            INSERT INTO user_summon_pools
                (user_id, pool_id, online_time, offline_time, discount_time, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, pool_id) DO UPDATE SET
                online_time = excluded.online_time,
                offline_time = excluded.offline_time,
                discount_time = excluded.discount_time,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(user_id)
        .bind(visible_pool.pool_id)
        .bind(visible_pool.online_time)
        .bind(visible_pool.offline_time)
        .bind(visible_pool.discount_time)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn visible_pools() -> Vec<VisibleSummonPool> {
    let mut visible = visible_scheduled_pools(ServerTime::now_sec_i32());
    visible.entry(1).or_insert(VisibleSummonPool {
        pool_id: 1,
        online_time: 0,
        offline_time: i32::MAX,
        discount_time: 0,
    });
    visible.entry(2).or_insert(VisibleSummonPool {
        pool_id: 2,
        online_time: 0,
        offline_time: i32::MAX,
        discount_time: 0,
    });

    visible.into_values().collect()
}

fn visible_scheduled_pools(now_sec: i32) -> BTreeMap<i32, VisibleSummonPool> {
    let mut pools = scheduled_pools();
    let open = pools
        .iter()
        .filter(|pool| pool.online_time <= now_sec && now_sec <= pool.offline_time)
        .cloned()
        .collect::<Vec<_>>();

    if !open.is_empty() {
        pools = open;
    } else {
        for pool in &mut pools {
            pool.online_time = (now_sec - 3600).max(0);
            pool.offline_time = now_sec.saturating_add(30 * 24 * 60 * 60);
        }
    }

    pools.into_iter().map(|pool| (pool.pool_id, pool)).collect()
}

fn scheduled_pools() -> Vec<VisibleSummonPool> {
    let tables = config::configs::get();
    let current_pool_ids = tables
        .current_summon_pools()
        .map(|pool| pool.id)
        .collect::<HashSet<_>>();
    let mut by_pool = BTreeMap::<i32, VisibleSummonPool>::new();
    for store in tables
        .store_recommend
        .iter()
        .filter(|store| store.is_offline == 0)
    {
        let Some(pool_id) = parse_pool_relation(&store.relations) else {
            continue;
        };
        if !current_pool_ids.contains(&pool_id) {
            continue;
        }
        let Some(pool) = tables.summon_pool.get(pool_id) else {
            continue;
        };
        let next = VisibleSummonPool {
            pool_id,
            online_time: parse_ts_seconds(&store.online_time).unwrap_or(0),
            offline_time: parse_ts_seconds(&store.offline_time).unwrap_or(0),
            discount_time: pool.discount_time10,
        };
        by_pool.entry(pool_id).or_insert(next);
    }

    by_pool.into_values().collect()
}

fn parse_pool_relation(relations: &str) -> Option<i32> {
    relations
        .split('|')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("1#")?.parse::<i32>().ok())
}

fn parse_ts_seconds(s: &str) -> Option<i32> {
    let dt = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
    Some(Utc.from_utc_datetime(&dt).timestamp() as i32)
}

async fn load_all_lucky_bags(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<HashMap<i32, LuckyBagInfo>> {
    let bags: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT pool_id, count, not_ssr_count FROM user_lucky_bags WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let single_bags: Vec<(i32, i32, bool)> = sqlx::query_as(
        "SELECT pool_id, bag_id, is_open FROM user_single_bags WHERE user_id = ? ORDER BY bag_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut singles_by_pool: HashMap<i32, Vec<SingleBagInfo>> = HashMap::new();
    for (pid, bag_id, is_open) in single_bags {
        singles_by_pool
            .entry(pid)
            .or_default()
            .push(SingleBagInfo { bag_id, is_open });
    }

    Ok(bags
        .into_iter()
        .map(|(pool_id, count, not_ssr_count)| {
            (
                pool_id,
                LuckyBagInfo {
                    count,
                    not_ssr_count,
                    single_bag_infos: singles_by_pool.remove(&pool_id).unwrap_or_default(),
                },
            )
        })
        .collect())
}

async fn load_all_sp_pools(pool: &SqlitePool, user_id: i64) -> Result<HashMap<i32, SpPoolInfo>> {
    let rows: Vec<(i32, i32, i32, i32, i64, bool, i32)> = sqlx::query_as(
        "SELECT pool_id, sp_type, limited_ticket_id, limited_ticket_num,
                open_time, used_first_ssr_guarantee, infallible_item_status
         FROM user_sp_pool_info WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let up_heroes: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT pool_id, hero_id FROM user_sp_pool_up_heroes WHERE user_id = ? ORDER BY hero_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let reward_progresses: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT pool_id, progress_id FROM user_sp_pool_reward_progress WHERE user_id = ? ORDER BY progress_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut heroes_by_pool: HashMap<i32, Vec<i32>> = HashMap::new();
    for (pid, hero_id) in up_heroes {
        heroes_by_pool.entry(pid).or_default().push(hero_id);
    }

    let mut progress_by_pool: HashMap<i32, Vec<i32>> = HashMap::new();
    for (pid, progress_id) in reward_progresses {
        progress_by_pool.entry(pid).or_default().push(progress_id);
    }

    Ok(rows
        .into_iter()
        .map(
            |(
                pool_id,
                sp_type,
                limited_ticket_id,
                limited_ticket_num,
                open_time,
                used_first_ssr_guarantee,
                infallible_item_status,
            )| {
                (
                    pool_id,
                    SpPoolInfo {
                        sp_type,
                        limited_ticket_id,
                        limited_ticket_num,
                        open_time: open_time as u64,
                        used_first_ssr_guarantee,
                        infallible_item_status,
                        up_hero_ids: heroes_by_pool.remove(&pool_id).unwrap_or_default(),
                        has_get_reward_progresses: progress_by_pool
                            .remove(&pool_id)
                            .unwrap_or_default(),
                    },
                )
            },
        )
        .collect())
}

async fn load_all_pop_up_infos(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<HashMap<i32, Vec<PopUpInfo>>> {
    let rows: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT pool_id, order_id, recommend_pop_up_count
         FROM user_pool_pop_up_infos WHERE user_id = ? ORDER BY pool_id, order_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<i32, Vec<PopUpInfo>> = HashMap::new();
    for (pool_id, order_id, recommend_pop_up_count) in rows {
        map.entry(pool_id).or_default().push(PopUpInfo {
            order_id,
            recommend_pop_up_count,
        });
    }
    Ok(map)
}

pub async fn increment_recommend_pop_up_count(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    order_id: i32,
) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "INSERT INTO user_pool_pop_up_infos
             (user_id, pool_id, order_id, recommend_pop_up_count)
         VALUES (?, ?, ?, 1)
         ON CONFLICT(user_id, pool_id, order_id) DO UPDATE SET
             recommend_pop_up_count = recommend_pop_up_count + 1
         RETURNING recommend_pop_up_count",
    )
    .bind(user_id)
    .bind(pool_id)
    .bind(order_id)
    .fetch_one(pool)
    .await?)
}

pub async fn get_sp_pool_info(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
) -> Result<Option<SpPoolInfo>> {
    let sp_data: Option<(i32, i32, i32, i64, bool, i32)> = sqlx::query_as(
        "SELECT sp_type, limited_ticket_id, limited_ticket_num, open_time, used_first_ssr_guarantee, infallible_item_status
         FROM user_sp_pool_info WHERE user_id = ? AND pool_id = ?",
    )
    .bind(user_id)
    .bind(pool_id)
    .fetch_optional(pool)
    .await?;

    if let Some((
        sp_type,
        limited_ticket_id,
        limited_ticket_num,
        open_time,
        used_first_ssr_guarantee,
        infallible_item_status,
    )) = sp_data
    {
        let up_hero_ids = sqlx::query_scalar(
            "SELECT hero_id FROM user_sp_pool_up_heroes WHERE user_id = ? AND pool_id = ? ORDER BY hero_id"
        )
        .bind(user_id)
        .bind(pool_id)
        .fetch_all(pool)
        .await?;

        let has_get_reward_progresses = sqlx::query_scalar(
            "SELECT progress_id FROM user_sp_pool_reward_progress WHERE user_id = ? AND pool_id = ? ORDER BY progress_id"
        )
        .bind(user_id)
        .bind(pool_id)
        .fetch_all(pool)
        .await?;

        Ok(Some(SpPoolInfo {
            sp_type,
            up_hero_ids,
            limited_ticket_id,
            limited_ticket_num,
            open_time: open_time as u64,
            used_first_ssr_guarantee,
            has_get_reward_progresses,
            infallible_item_status,
        }))
    } else {
        Ok(None)
    }
}

pub async fn add_summon_history(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    pool_name: String,
    pool_type: i32,
    summon_type: i32,
    results: Vec<SummonResult>,
) -> sqlx::Result<()> {
    let now = common::time::ServerTime::now_ms();

    // Insert summon history row
    let history_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO user_summon_history (
            user_id, pool_id, summon_type, pool_type, pool_name, summon_time
        )
        VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(pool_id)
    .bind(summon_type)
    .bind(pool_type)
    .bind(pool_name)
    .bind(now)
    .fetch_one(pool)
    .await?;

    // Insert gained items (heroes from results)
    for (idx, result) in results.iter().enumerate() {
        if let Some(hero_id) = result.hero_id {
            // Insert hero result
            sqlx::query(
                r#"
                INSERT INTO user_summon_history_items (
                    history_id, result_index, gain_id
                )
                VALUES (?, ?, ?)
                "#,
            )
            .bind(history_id)
            .bind(idx as i32)
            .bind(hero_id)
            .execute(pool)
            .await?;
        }
    }

    tracing::debug!(
        "Inserted summon history for user {}: pool {}, {} results",
        user_id,
        pool_id,
        results.len()
    );

    Ok(())
}

pub async fn add_summon_history_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    pool_id: i32,
    pool_name: &str,
    pool_type: i32,
    summon_type: i32,
    results: &[SummonResult],
) -> sqlx::Result<()> {
    let history_id: i64 = sqlx::query_scalar(
        "INSERT INTO user_summon_history
             (user_id, pool_id, summon_type, pool_type, pool_name, summon_time)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(user_id)
    .bind(pool_id)
    .bind(summon_type)
    .bind(pool_type)
    .bind(pool_name)
    .bind(ServerTime::now_ms())
    .fetch_one(&mut **tx)
    .await?;
    for (index, result) in results.iter().enumerate() {
        if let Some(hero_id) = result.hero_id {
            sqlx::query(
                "INSERT INTO user_summon_history_items
                     (history_id, result_index, gain_id)
                 VALUES (?, ?, ?)",
            )
            .bind(history_id)
            .bind(index as i32)
            .bind(hero_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

pub async fn update_sp_pool_up_heroes(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    up_hero_ids: Vec<i32>,
) -> Result<()> {
    let sp_type = config::configs::get()
        .summon_pool
        .get(pool_id)
        .map(|pool| pool.r#type)
        .unwrap_or_default();
    ensure_sp_pool_info(pool, user_id, pool_id, sp_type).await?;

    sqlx::query(
        r#"
        DELETE FROM user_sp_pool_up_heroes
        WHERE user_id = ? AND pool_id = ?
        "#,
    )
    .bind(user_id)
    .bind(pool_id)
    .execute(pool)
    .await?;

    for hero_id in up_hero_ids {
        sqlx::query(
            r#"
            INSERT INTO user_sp_pool_up_heroes (user_id, pool_id, hero_id)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(pool_id)
        .bind(hero_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn ensure_sp_pool_info(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    sp_type: i32,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_sp_pool_info
         (user_id, pool_id, sp_type, limited_ticket_id, limited_ticket_num,
          open_time, used_first_ssr_guarantee, infallible_item_status)
         VALUES (?, ?, ?, 0, 0, 0, 0, 0)
         ON CONFLICT(user_id, pool_id) DO UPDATE SET
             sp_type = excluded.sp_type",
    )
    .bind(user_id)
    .bind(pool_id)
    .bind(sp_type)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn use_discount(pool: &SqlitePool, user_id: i64, pool_id: i32) -> Result<()> {
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        "UPDATE user_summon_pools
         SET discount_time = discount_time - 1, updated_at = ?
         WHERE user_id = ? AND pool_id = ? AND discount_time > 0",
    )
    .bind(now)
    .bind(user_id)
    .bind(pool_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn increment_summon_count(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    pool_id: i32,
    count: i32,
) -> Result<()> {
    let now = ServerTime::now_ms();
    let summon_pool = config::configs::get()
        .summon_pool
        .get(pool_id)
        .ok_or_else(|| anyhow::anyhow!("Summon pool {} not found", pool_id))?;
    let pool_ids = if summon_pool.r#type == 3 {
        config::configs::get()
            .summon_pool
            .iter()
            .filter(|pool| pool.r#type == 3)
            .map(|pool| pool.id)
            .collect()
    } else {
        vec![pool_id]
    };
    for current_pool_id in pool_ids {
        sqlx::query(
            "INSERT INTO user_summon_pools
                 (user_id, pool_id, summon_count, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(user_id, pool_id) DO UPDATE SET
                 summon_count = summon_count + excluded.summon_count,
                 updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(current_pool_id)
        .bind(count)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn record_summon(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    count: i32,
    is_newbie_pool: bool,
    completed_newbie_pool: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_summon_stats
             (user_id, is_show_new_summon)
         VALUES (?, 1)",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE user_summon_stats
         SET total_summon_count = total_summon_count + ?,
             new_summon_count = new_summon_count
                 + CASE WHEN is_show_new_summon AND ? THEN ? ELSE 0 END,
             is_show_new_summon
                 = CASE WHEN ? THEN 0 ELSE is_show_new_summon END
         WHERE user_id = ?",
    )
    .bind(count)
    .bind(is_newbie_pool)
    .bind(count)
    .bind(completed_newbie_pool)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn claim_progress_rewards(
    pool: &SqlitePool,
    user_id: i64,
    pool_id: i32,
    rewards: &[(i32, i32, i32)],
) -> Result<(Vec<i32>, Vec<i32>)> {
    let mut tx = pool.begin().await?;
    let summon_count: i32 = sqlx::query_scalar(
        "SELECT summon_count FROM user_summon_pools WHERE user_id = ? AND pool_id = ?",
    )
    .bind(user_id)
    .bind(pool_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_default();
    let mut changed_items = Vec::new();

    for &(progress, item_id, amount) in rewards {
        if progress > summon_count {
            continue;
        }
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO user_sp_pool_reward_progress (user_id, pool_id, progress_id)
             VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(pool_id)
        .bind(progress)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            continue;
        }

        sqlx::query(
            "INSERT INTO items (user_id, item_id, quantity, last_update_time, total_gain_count)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(user_id, item_id) DO UPDATE SET
                 quantity = quantity + excluded.quantity,
                 last_update_time = excluded.last_update_time,
                 total_gain_count = total_gain_count + excluded.total_gain_count",
        )
        .bind(user_id)
        .bind(item_id)
        .bind(amount)
        .bind(ServerTime::now_ms())
        .bind(amount)
        .execute(&mut *tx)
        .await?;
        changed_items.push(item_id);
    }

    let claimed = sqlx::query_scalar(
        "SELECT progress_id FROM user_sp_pool_reward_progress
         WHERE user_id = ? AND pool_id = ? ORDER BY progress_id",
    )
    .bind(user_id)
    .bind(pool_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok((claimed, changed_items))
}
