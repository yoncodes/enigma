use crate::models::game::rouge::{
    Rouge2AttrState, Rouge2CareerLevel, Rouge2MaterialState, Rouge2RewardState, Rouge2State,
};
use crate::{db::game::tasks::TaskEvent, models::game::achievements::Achievement};
use anyhow::Result;
use sqlx::SqlitePool;

pub struct Rouge2OutsideState {
    pub state: Rouge2State,
    pub attrs: Vec<Rouge2AttrState>,
    pub career_levels: Vec<Rouge2CareerLevel>,
    pub rewards: Vec<Rouge2RewardState>,
    pub materials: Vec<Rouge2MaterialState>,
}

pub struct DifficultyUpdate<'a> {
    pub difficulty: i32,
    pub attrs: &'a [(i32, i32)],
}

pub struct AlchemyUpdate<'a> {
    pub expected_state: i32,
    pub formula: i32,
    pub main_effect: i32,
    pub sub_effects: &'a str,
    pub genius_point_gain: i32,
    pub costs: &'a [(i32, i32)],
}

pub struct AlchemyApply {
    pub genius_point: i32,
    pub materials: Vec<Rouge2MaterialState>,
    pub achievements: Vec<Achievement>,
}

#[derive(Clone, Copy)]
pub enum Rouge2UnlockKind {
    Relic,
    Buff,
    ActiveSkill,
}

impl Rouge2UnlockKind {
    const fn id(self) -> i32 {
        match self {
            Self::Relic => 1,
            Self::Buff => 2,
            Self::ActiveSkill => 3,
        }
    }
}

pub async fn get_or_create_rouge2_outside(
    pool: &SqlitePool,
    user_id: i64,
    tables: &config::GameDB,
) -> Result<Rouge2OutsideState> {
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        "INSERT INTO user_rouge2_state (user_id, updated_at)
         VALUES (?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(now)
    .execute(pool)
    .await?;

    for career in tables.rouge2_career.iter() {
        sqlx::query(
            "INSERT INTO user_rouge2_career_levels (user_id, career_id, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(career.id)
        .bind(now)
        .execute(pool)
        .await?;
    }

    for reward in tables.rouge2_reward.iter() {
        sqlx::query(
            "INSERT INTO user_rouge2_rewards (user_id, reward_id, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(reward.id)
        .bind(now)
        .execute(pool)
        .await?;
    }

    let state = sqlx::query_as::<_, Rouge2State>(
        "SELECT user_id, state, difficulty, coin, end_id, game_num,
                genius_point, genius_ids, reward_point, max_difficulty,
                pass_layer_ids, pass_event_ids, pass_end_ids, pass_entrust_ids,
                pass_collections, last_game_time, hotfix_str,
                alchemy_formula, alchemy_main_effect, alchemy_sub_effects, updated_at
         FROM user_rouge2_state
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let career_levels = sqlx::query_as::<_, Rouge2CareerLevel>(
        "SELECT user_id, career_id, exp, updated_at
         FROM user_rouge2_career_levels
         WHERE user_id = ?
         ORDER BY career_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let attrs = sqlx::query_as::<_, Rouge2AttrState>(
        "SELECT user_id, attr_id, final_val, updated_at
         FROM user_rouge2_attrs
         WHERE user_id = ?
         ORDER BY rowid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let rewards = sqlx::query_as::<_, Rouge2RewardState>(
        "SELECT user_id, reward_id, buy_count, updated_at
         FROM user_rouge2_rewards
         WHERE user_id = ?
         ORDER BY reward_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let materials = sqlx::query_as::<_, Rouge2MaterialState>(
        "SELECT user_id, material_id, num, is_seen, updated_at
         FROM user_rouge2_materials
         WHERE user_id = ? AND is_seen = 1
         ORDER BY rowid",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(Rouge2OutsideState {
        state,
        attrs,
        career_levels,
        rewards,
        materials,
    })
}

pub async fn apply_difficulty(
    pool: &SqlitePool,
    user_id: i64,
    update: DifficultyUpdate<'_>,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE user_rouge2_state
         SET state = 1, difficulty = ?, coin = 0, end_id = 0,
             game_num = game_num + 1, updated_at = ?
         WHERE user_id = ? AND state = 0",
    )
    .bind(update.difficulty)
    .bind(now)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if changed.rows_affected() != 1 {
        return Ok(false);
    }

    sqlx::query("DELETE FROM user_rouge2_attrs WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for &(attr_id, final_val) in update.attrs {
        sqlx::query(
            "INSERT INTO user_rouge2_attrs
             (user_id, attr_id, final_val, updated_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(attr_id)
        .bind(final_val)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(true)
}

pub async fn apply_alchemy(
    pool: &SqlitePool,
    user_id: i64,
    update: AlchemyUpdate<'_>,
) -> Result<Option<AlchemyApply>> {
    let now = common::time::ServerTime::now_ms();
    let mut tx = pool.begin().await?;

    for &(material_id, amount) in update.costs {
        let changed = sqlx::query(
            "UPDATE user_rouge2_materials
             SET num = num - ?, is_seen = 1, updated_at = ?
             WHERE user_id = ? AND material_id = ? AND num >= ?",
        )
        .bind(amount)
        .bind(now)
        .bind(user_id)
        .bind(material_id)
        .bind(amount)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Ok(None);
        }
    }

    let genius_point = sqlx::query_scalar::<_, i32>(
        "UPDATE user_rouge2_state
         SET alchemy_formula = ?, alchemy_main_effect = ?, alchemy_sub_effects = ?,
             genius_point = genius_point + ?, updated_at = ?
         WHERE user_id = ? AND state = ? AND alchemy_formula = 0
         RETURNING genius_point",
    )
    .bind(update.formula)
    .bind(update.main_effect)
    .bind(update.sub_effects)
    .bind(update.genius_point_gain)
    .bind(now)
    .bind(user_id)
    .bind(update.expected_state)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(genius_point) = genius_point else {
        return Ok(None);
    };

    let achievements = super::achievements::sync_event_in_transaction(
        &mut tx,
        user_id,
        TaskEvent::Rouge2Alchemy { count: 1 },
    )
    .await?;
    let materials = sqlx::query_as::<_, Rouge2MaterialState>(
        "SELECT user_id, material_id, num, is_seen, updated_at
         FROM user_rouge2_materials
         WHERE user_id = ? AND is_seen = 1
         ORDER BY rowid",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some(AlchemyApply {
        genius_point,
        materials,
        achievements,
    }))
}

pub async fn get_or_create_unlock_ids(
    pool: &SqlitePool,
    user_id: i64,
    tables: &config::GameDB,
    kind: Rouge2UnlockKind,
) -> Result<Vec<i32>> {
    let now = common::time::ServerTime::now_ms();
    let ids: Vec<i32> = match kind {
        Rouge2UnlockKind::Relic => tables.rouge2_relics.iter().map(|row| row.id).collect(),
        Rouge2UnlockKind::Buff => tables.rouge2_buff.iter().map(|row| row.id).collect(),
        Rouge2UnlockKind::ActiveSkill => tables
            .rouge2_active_skill
            .iter()
            .map(|row| row.id)
            .collect(),
    };

    for id in ids {
        sqlx::query(
            "INSERT INTO user_rouge2_unlocks (user_id, unlock_type, unlock_id, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(kind.id())
        .bind(id)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(sqlx::query_scalar(
        "SELECT unlock_id
         FROM user_rouge2_unlocks
         WHERE user_id = ? AND unlock_type = ?
         ORDER BY unlock_id",
    )
    .bind(user_id)
    .bind(kind.id())
    .fetch_all(pool)
    .await?)
}

#[cfg(test)]
mod tests;
