use anyhow::{Result, bail};
use sqlx::SqlitePool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeekwalkV2BattleSeed {
    pub battle_id: i32,
    pub element_id: i32,
    pub index: i32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeekwalkV2LayerSeed {
    pub layer_id: i32,
    pub scene_id: i32,
    pub unlock: bool,
    pub battles: Vec<WeekwalkV2BattleSeed>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeekwalkV2Seed {
    pub time_id: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub pop_rule: bool,
    pub layers: Vec<WeekwalkV2LayerSeed>,
    pub previous_layer_ids: Vec<i32>,
}

pub async fn ensure_weekwalk_v2_state(
    pool: &SqlitePool,
    user_id: i64,
    seed: &WeekwalkV2Seed,
) -> Result<()> {
    let current: Option<i32> =
        sqlx::query_scalar("SELECT time_id FROM user_weekwalk_v2_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    if let Some(current) = current {
        if current == seed.time_id {
            return Ok(());
        }
        bail!(
            "Weekwalk v2 rollover from time {current} to {} requires settlement",
            seed.time_id
        );
    }

    let mut tx = pool.begin().await?;
    for table in [
        "user_weekwalk_v2_snapshot_skills",
        "user_weekwalk_v2_snapshots",
        "user_weekwalk_v2_prev_settle_layers",
        "user_weekwalk_v2_prev_settle",
        "user_weekwalk_v2_elements",
        "user_weekwalk_v2_cups",
        "user_weekwalk_v2_battle_skills",
        "user_weekwalk_v2_battle_heroes",
        "user_weekwalk_v2_battles",
        "user_weekwalk_v2_layers",
        "user_weekwalk_v2_info",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE user_id = ?"))
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        "INSERT INTO user_weekwalk_v2_info
         (user_id, time_id, start_time, end_time, pop_rule) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(seed.time_id)
    .bind(seed.start_time)
    .bind(seed.end_time)
    .bind(seed.pop_rule)
    .execute(&mut *tx)
    .await?;

    for layer in &seed.layers {
        sqlx::query(
            "INSERT INTO user_weekwalk_v2_layers
             (user_id, layer_id, scene_id, all_pass, finished, unlock, show_finished, params)
             VALUES (?, ?, ?, 0, 0, ?, 0, '')",
        )
        .bind(user_id)
        .bind(layer.layer_id)
        .bind(layer.scene_id)
        .bind(layer.unlock)
        .execute(&mut *tx)
        .await?;

        for battle in &layer.battles {
            sqlx::query(
                "INSERT INTO user_weekwalk_v2_battles
                 (user_id, layer_id, battle_id, status, hero_group_select, element_id, params)
                 VALUES (?, ?, ?, 0, 0, ?, '')",
            )
            .bind(user_id)
            .bind(layer.layer_id)
            .bind(battle.battle_id)
            .bind(battle.element_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO user_weekwalk_v2_elements
                 (user_id, layer_id, element_id, finish, index_num, visible)
                 VALUES (?, ?, ?, 0, ?, ?)",
            )
            .bind(user_id)
            .bind(layer.layer_id)
            .bind(battle.element_id)
            .bind(battle.index)
            .bind(battle.visible)
            .execute(&mut *tx)
            .await?;
        }
    }

    sqlx::query(
        "INSERT INTO user_weekwalk_v2_prev_settle
         (user_id, max_layer_id, max_battle_id, max_battle_index, show)
         VALUES (?, 0, 0, 0, 0)",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    for layer_id in &seed.previous_layer_ids {
        sqlx::query(
            "INSERT INTO user_weekwalk_v2_prev_settle_layers
             (user_id, layer_id, platinum_cup_num) VALUES (?, ?, 0)",
        )
        .bind(user_id)
        .bind(layer_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn mark_layer_finished_shown(
    pool: &SqlitePool,
    user_id: i64,
    layer_id: i32,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET show_finished = 1
         WHERE user_id = ? AND layer_id = ? AND all_pass = 1",
    )
    .bind(user_id)
    .bind(layer_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn set_battle_group(
    pool: &SqlitePool,
    user_id: i64,
    layer_id: i32,
    battle_id: i32,
    select: i32,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_weekwalk_v2_battles SET hero_group_select = ?
         WHERE user_id = ? AND layer_id = ? AND battle_id = ? AND status = 0
           AND EXISTS (
               SELECT 1 FROM user_weekwalk_v2_layers AS layer
               WHERE layer.user_id = user_weekwalk_v2_battles.user_id
                 AND layer.layer_id = user_weekwalk_v2_battles.layer_id
                 AND layer.unlock = 1
           )
           AND EXISTS (
               SELECT 1 FROM user_weekwalk_v2_elements AS element
               WHERE element.user_id = user_weekwalk_v2_battles.user_id
                 AND element.layer_id = user_weekwalk_v2_battles.layer_id
                 AND element.element_id = user_weekwalk_v2_battles.element_id
                 AND element.visible = 1 AND element.finish = 0
           )",
    )
    .bind(select)
    .bind(user_id)
    .bind(layer_id)
    .bind(battle_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
