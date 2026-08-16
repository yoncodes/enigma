use anyhow::Result;
use sonettobuf::BpSelfSelectBonus;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone)]
pub struct BattlePassState {
    pub score: i32,
    pub weekly_score: i32,
    pub pay_status: i32,
    pub first_show: bool,
    pub sp_first_show: bool,
    pub has_get_self_select_bonus: Vec<BpSelfSelectBonus>,
    pub has_get_free_bonus: Vec<i32>,
    pub has_get_pay_bonus: Vec<i32>,
    pub has_get_sp_free_bonus: Vec<i32>,
    pub has_get_sp_pay_bonus: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct BattlePassPurchaseUpdate {
    pub score: i32,
    pub weekly_score: i32,
    pub pay_status: i32,
    pub previous_pay_status: i32,
    pub score_changed: bool,
    pub pay_status_changed: bool,
}

pub struct BattlePassClaimLevels<'a> {
    pub free: &'a [i32],
    pub pay: &'a [i32],
    pub sp_free: &'a [i32],
    pub sp_pay: &'a [i32],
}

pub async fn get_or_create_state(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
) -> Result<BattlePassState> {
    sqlx::query(
        r#"
        DELETE FROM user_battle_pass_state
        WHERE user_id = ? AND bp_id <> ?
        "#,
    )
    .bind(user_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_battle_pass_state (user_id, bp_id)
        VALUES (?, ?)
        "#,
    )
    .bind(user_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    get_state(pool, user_id, bp_id).await
}

pub async fn get_state(pool: &SqlitePool, user_id: i64, bp_id: i32) -> Result<BattlePassState> {
    let (
        score,
        weekly_score,
        pay_status,
        first_show,
        sp_first_show,
        self_select_json,
        free_json,
        pay_json,
        sp_free_json,
        sp_pay_json,
    ) = sqlx::query_as::<_, (i32, i32, i32, bool, bool, String, String, String, String, String)>(
            r#"
            SELECT score, weekly_score, pay_status, first_show, sp_first_show, has_get_self_select_bonus,
                   has_get_free_bonus, has_get_pay_bonus, has_get_sp_free_bonus, has_get_sp_pay_bonus
            FROM user_battle_pass_state
            WHERE user_id = ? AND bp_id = ?
            "#,
        )
        .bind(user_id)
        .bind(bp_id)
        .fetch_one(pool)
        .await?;

    Ok(BattlePassState {
        score,
        weekly_score,
        pay_status,
        first_show,
        sp_first_show,
        has_get_self_select_bonus: serde_json::from_str(&self_select_json)?,
        has_get_free_bonus: serde_json::from_str(&free_json)?,
        has_get_pay_bonus: serde_json::from_str(&pay_json)?,
        has_get_sp_free_bonus: serde_json::from_str(&sp_free_json)?,
        has_get_sp_pay_bonus: serde_json::from_str(&sp_pay_json)?,
    })
}

pub async fn claim_bonus_levels(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    free_levels: &[i32],
    pay_levels: &[i32],
    sp_free_levels: &[i32],
    sp_pay_levels: &[i32],
) -> Result<BattlePassState> {
    let mut state = get_or_create_state(pool, user_id, bp_id).await?;
    extend_unique(&mut state.has_get_free_bonus, free_levels);
    extend_unique(&mut state.has_get_pay_bonus, pay_levels);
    extend_unique(&mut state.has_get_sp_free_bonus, sp_free_levels);
    extend_unique(&mut state.has_get_sp_pay_bonus, sp_pay_levels);

    sqlx::query(
        r#"
        UPDATE user_battle_pass_state
        SET has_get_free_bonus = ?,
            has_get_pay_bonus = ?,
            has_get_sp_free_bonus = ?,
            has_get_sp_pay_bonus = ?,
            updated_at = ?
        WHERE user_id = ? AND bp_id = ?
        "#,
    )
    .bind(serde_json::to_string(&state.has_get_free_bonus)?)
    .bind(serde_json::to_string(&state.has_get_pay_bonus)?)
    .bind(serde_json::to_string(&state.has_get_sp_free_bonus)?)
    .bind(serde_json::to_string(&state.has_get_sp_pay_bonus)?)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    Ok(state)
}

pub async fn claim_bonus_levels_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bp_id: i32,
    current: &BattlePassState,
    levels: BattlePassClaimLevels<'_>,
) -> Result<Option<BattlePassState>> {
    let mut state = current.clone();
    extend_unique(&mut state.has_get_free_bonus, levels.free);
    extend_unique(&mut state.has_get_pay_bonus, levels.pay);
    extend_unique(&mut state.has_get_sp_free_bonus, levels.sp_free);
    extend_unique(&mut state.has_get_sp_pay_bonus, levels.sp_pay);

    let result = sqlx::query(
        "UPDATE user_battle_pass_state
         SET has_get_free_bonus = ?, has_get_pay_bonus = ?,
             has_get_sp_free_bonus = ?, has_get_sp_pay_bonus = ?, updated_at = ?
         WHERE user_id = ? AND bp_id = ?
           AND has_get_free_bonus = ? AND has_get_pay_bonus = ?
           AND has_get_sp_free_bonus = ? AND has_get_sp_pay_bonus = ?",
    )
    .bind(serde_json::to_string(&state.has_get_free_bonus)?)
    .bind(serde_json::to_string(&state.has_get_pay_bonus)?)
    .bind(serde_json::to_string(&state.has_get_sp_free_bonus)?)
    .bind(serde_json::to_string(&state.has_get_sp_pay_bonus)?)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .bind(serde_json::to_string(&current.has_get_free_bonus)?)
    .bind(serde_json::to_string(&current.has_get_pay_bonus)?)
    .bind(serde_json::to_string(&current.has_get_sp_free_bonus)?)
    .bind(serde_json::to_string(&current.has_get_sp_pay_bonus)?)
    .execute(&mut **tx)
    .await?;

    Ok((result.rows_affected() == 1).then_some(state))
}

pub async fn claim_self_select_bonus(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    level: i32,
    index: i32,
) -> Result<BattlePassState> {
    let mut state = get_or_create_state(pool, user_id, bp_id).await?;
    if state
        .has_get_self_select_bonus
        .iter()
        .all(|bonus| bonus.level != Some(level))
    {
        state.has_get_self_select_bonus.push(BpSelfSelectBonus {
            level: Some(level),
            index: Some(index),
        });
        sqlx::query(
            "UPDATE user_battle_pass_state SET has_get_self_select_bonus = ?, updated_at = ? \
             WHERE user_id = ? AND bp_id = ?",
        )
        .bind(serde_json::to_string(&state.has_get_self_select_bonus)?)
        .bind(common::time::ServerTime::now_ms())
        .bind(user_id)
        .bind(bp_id)
        .execute(pool)
        .await?;
    }

    Ok(state)
}

pub async fn claim_self_select_bonus_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bp_id: i32,
    current: &BattlePassState,
    level: i32,
    index: i32,
) -> Result<Option<BattlePassState>> {
    if current
        .has_get_self_select_bonus
        .iter()
        .any(|bonus| bonus.level == Some(level))
    {
        return Ok(None);
    }

    let mut state = current.clone();
    state.has_get_self_select_bonus.push(BpSelfSelectBonus {
        level: Some(level),
        index: Some(index),
    });
    let old = serde_json::to_string(&current.has_get_self_select_bonus)?;
    let result = sqlx::query(
        "UPDATE user_battle_pass_state
         SET has_get_self_select_bonus = ?, updated_at = ?
         WHERE user_id = ? AND bp_id = ? AND has_get_self_select_bonus = ?",
    )
    .bind(serde_json::to_string(&state.has_get_self_select_bonus)?)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .bind(old)
    .execute(&mut **tx)
    .await?;
    Ok((result.rows_affected() == 1).then_some(state))
}

pub async fn buy_levels(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    currency_id: i32,
    currency_cost: i32,
    score_delta: i32,
) -> Result<Option<i32>> {
    get_or_create_state(pool, user_id, bp_id).await?;
    let mut transaction = pool.begin().await?;
    let charged = sqlx::query(
        "UPDATE currencies SET quantity = quantity - ?, last_recover_time = ? \
         WHERE user_id = ? AND currency_id = ? AND quantity >= ?",
    )
    .bind(currency_cost)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(currency_id)
    .bind(currency_cost)
    .execute(&mut *transaction)
    .await?;
    if charged.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }

    sqlx::query(
        "UPDATE user_battle_pass_state SET score = score + ?, updated_at = ? \
         WHERE user_id = ? AND bp_id = ?",
    )
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .execute(&mut *transaction)
    .await?;
    let score = sqlx::query_scalar(
        "SELECT score FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Some(score))
}

pub async fn mark_first_show(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    is_sp: bool,
) -> Result<()> {
    get_or_create_state(pool, user_id, bp_id).await?;
    let column = if is_sp { "sp_first_show" } else { "first_show" };
    let query = format!(
        "UPDATE user_battle_pass_state SET {column} = 0, updated_at = ? WHERE user_id = ? AND bp_id = ?"
    );

    sqlx::query(&query)
        .bind(common::time::ServerTime::now_ms())
        .bind(user_id)
        .bind(bp_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn apply_purchase(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    pay_status: i32,
    score_delta: i32,
) -> Result<BattlePassPurchaseUpdate> {
    let before = get_or_create_state(pool, user_id, bp_id).await?;
    let pay_status_changed = before.pay_status < pay_status;
    let score_delta = if before.pay_status < pay_status {
        capped_score_delta(bp_id, before.score, score_delta)
    } else {
        0
    };

    sqlx::query(
        r#"
        UPDATE user_battle_pass_state
        SET pay_status = CASE WHEN pay_status < ? THEN ? ELSE pay_status END,
            score = score + ?,
            updated_at = ?
        WHERE user_id = ? AND bp_id = ?
        "#,
    )
    .bind(pay_status)
    .bind(pay_status)
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    let (score, weekly_score, pay_status) = sqlx::query_as::<_, (i32, i32, i32)>(
        "SELECT score, weekly_score, pay_status FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(pool)
    .await?;

    Ok(BattlePassPurchaseUpdate {
        score,
        weekly_score,
        pay_status,
        previous_pay_status: before.pay_status,
        score_changed: score_delta != 0,
        pay_status_changed,
    })
}

pub async fn apply_purchase_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bp_id: i32,
    pay_status: i32,
    score_delta: i32,
) -> Result<BattlePassPurchaseUpdate> {
    sqlx::query("DELETE FROM user_battle_pass_state WHERE user_id = ? AND bp_id <> ?")
        .bind(user_id)
        .bind(bp_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO user_battle_pass_state (user_id, bp_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(bp_id)
        .execute(&mut **tx)
        .await?;

    let (before_score, before_pay_status) = sqlx::query_as::<_, (i32, i32)>(
        "SELECT score, pay_status FROM user_battle_pass_state
         WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(&mut **tx)
    .await?;
    let pay_status_changed = before_pay_status < pay_status;
    let score_delta = if pay_status_changed {
        capped_score_delta(bp_id, before_score, score_delta)
    } else {
        0
    };

    sqlx::query(
        "UPDATE user_battle_pass_state
         SET pay_status = max(pay_status, ?), score = score + ?, updated_at = ?
         WHERE user_id = ? AND bp_id = ?",
    )
    .bind(pay_status)
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .execute(&mut **tx)
    .await?;

    let (score, weekly_score, pay_status) = sqlx::query_as::<_, (i32, i32, i32)>(
        "SELECT score, weekly_score, pay_status FROM user_battle_pass_state
         WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(BattlePassPurchaseUpdate {
        score,
        weekly_score,
        pay_status,
        previous_pay_status: before_pay_status,
        score_changed: score_delta != 0,
        pay_status_changed,
    })
}

pub async fn add_score(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
    score_delta: i32,
) -> Result<BattlePassPurchaseUpdate> {
    let before = get_or_create_state(pool, user_id, bp_id).await?;
    let score_delta = capped_score_delta(bp_id, before.score, score_delta);
    if score_delta == 0 {
        return Ok(BattlePassPurchaseUpdate {
            score: before.score,
            weekly_score: before.weekly_score,
            pay_status: before.pay_status,
            previous_pay_status: before.pay_status,
            score_changed: false,
            pay_status_changed: false,
        });
    }

    sqlx::query(
        r#"
        UPDATE user_battle_pass_state
        SET score = score + ?,
            weekly_score = weekly_score + ?,
            updated_at = ?
        WHERE user_id = ? AND bp_id = ?
        "#,
    )
    .bind(score_delta)
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    let (score, weekly_score, pay_status) = sqlx::query_as::<_, (i32, i32, i32)>(
        "SELECT score, weekly_score, pay_status FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(pool)
    .await?;

    Ok(BattlePassPurchaseUpdate {
        score,
        weekly_score,
        pay_status,
        previous_pay_status: before.pay_status,
        score_changed: true,
        pay_status_changed: false,
    })
}

pub async fn add_score_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bp_id: i32,
    score_delta: i32,
) -> Result<BattlePassPurchaseUpdate> {
    sqlx::query("DELETE FROM user_battle_pass_state WHERE user_id = ? AND bp_id <> ?")
        .bind(user_id)
        .bind(bp_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO user_battle_pass_state (user_id, bp_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(bp_id)
        .execute(&mut **tx)
        .await?;
    let before: (i32, i32, i32) = sqlx::query_as(
        "SELECT score, weekly_score, pay_status
         FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(&mut **tx)
    .await?;
    let score_delta = capped_score_delta(bp_id, before.0, score_delta);
    if score_delta == 0 {
        return Ok(BattlePassPurchaseUpdate {
            score: before.0,
            weekly_score: before.1,
            pay_status: before.2,
            previous_pay_status: before.2,
            score_changed: false,
            pay_status_changed: false,
        });
    }

    let (score, weekly_score): (i32, i32) = sqlx::query_as(
        "UPDATE user_battle_pass_state
         SET score = score + ?, weekly_score = weekly_score + ?, updated_at = ?
         WHERE user_id = ? AND bp_id = ?
         RETURNING score, weekly_score",
    )
    .bind(score_delta)
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(bp_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(BattlePassPurchaseUpdate {
        score,
        weekly_score,
        pay_status: before.2,
        previous_pay_status: before.2,
        score_changed: true,
        pay_status_changed: false,
    })
}

pub async fn score_maxed(pool: &SqlitePool, user_id: i64, bp_id: i32) -> sqlx::Result<bool> {
    let score: i32 = sqlx::query_scalar(
        "SELECT score FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    Ok(max_score(bp_id).is_some_and(|max| score >= max))
}

pub async fn score_maxed_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<bool> {
    let score: i32 = sqlx::query_scalar(
        "SELECT score FROM user_battle_pass_state WHERE user_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(bp_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default();

    Ok(max_score(bp_id).is_some_and(|max| score >= max))
}

fn capped_score_delta(bp_id: i32, current_score: i32, score_delta: i32) -> i32 {
    let score_delta = score_delta.max(0);
    let Some(max_score) = max_score(bp_id) else {
        return score_delta;
    };

    (max_score - current_score).clamp(0, score_delta)
}

fn max_score(bp_id: i32) -> Option<i32> {
    let tables = config::configs::get();
    let bp = tables.battle_pass(bp_id)?;
    let max_level = tables
        .battle_pass_bonuses(bp_id)
        .map(|bonus| bonus.level)
        .max()?;
    Some(max_level * bp.exp_level_up.max(1))
}

fn extend_unique(values: &mut Vec<i32>, new_values: &[i32]) {
    for value in new_values {
        if !values.contains(value) {
            values.push(*value);
        }
    }
}
