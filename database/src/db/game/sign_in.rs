use crate::{
    db::game::tasks,
    models::game::sign_in::{MonthCardHistory, UserSignInInfo},
};
use anyhow::Result;
use chrono::Datelike;
use common::time::ServerTime;
use sonettobuf::MonthCardInfo;
use sqlx::{Sqlite, SqlitePool, Transaction};

const DAY_SECONDS: i32 = 24 * 60 * 60;
const HERO_TOUCH_COUNT_CONFIG_ID: i32 = 32;

/// Record sign-in for today
async fn record_sign_in_day(pool: &SqlitePool, user_id: i64, now: i64) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let recorded = record_sign_in_day_in_transaction(&mut tx, user_id, now).await?;
    tx.commit().await?;
    Ok(recorded)
}

pub async fn record_sign_in_day_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    now: i64,
) -> Result<bool> {
    let server_day = ServerTime::server_day(now);
    let day_of_month = ServerTime::day_of_month(now) as i32;
    let inserted = sqlx::query(
        "INSERT INTO user_sign_in_days (user_id, server_day, day_of_month)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, server_day) DO NOTHING",
    )
    .bind(user_id)
    .bind(server_day)
    .bind(day_of_month)
    .execute(&mut **tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query(
        r#"
        INSERT INTO user_sign_in_info
            (user_id, addup_sign_in_day, open_function_time, reward_mark)
        VALUES (?, 1, ?, 0)
        ON CONFLICT(user_id)
        DO UPDATE SET addup_sign_in_day = addup_sign_in_day + 1
        "#,
    )
    .bind(user_id)
    .bind(now / 1000)
    .execute(&mut **tx)
    .await?;

    tracing::info!(
        "Sign-in recorded user_id={} day={} server_day={}",
        user_id,
        day_of_month,
        server_day
    );

    Ok(true)
}

/// Process daily login - returns (is_new_day, is_new_week, is_new_month)
pub async fn process_daily_login(pool: &SqlitePool, user_id: i64) -> Result<(bool, bool, bool)> {
    let now = ServerTime::now_ms();

    let users_rows = sqlx::query("UPDATE users SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();

    if users_rows == 0 {
        anyhow::bail!("users row missing for user_id={}", user_id);
    }

    let last_sign_in_time: Option<i64> =
        sqlx::query_scalar("SELECT last_sign_in_time FROM player_state WHERE player_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    let is_new_day = match last_sign_in_time {
        Some(last) if last > 0 => ServerTime::is_new_day(last, now),
        _ => true,
    };

    let is_new_week = match last_sign_in_time {
        Some(last) if last > 0 => !ServerTime::is_same_week(last, now),
        _ => true,
    };

    let is_new_month = match last_sign_in_time {
        Some(last) if last > 0 => !ServerTime::is_same_month(last, now),
        _ => true,
    };

    if is_new_month {
        sqlx::query("DELETE FROM user_sign_in_days WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;

        sqlx::query("DELETE FROM user_month_card_days WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;

        sqlx::query("DELETE FROM user_sign_in_addup_bonus WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;

        reset_monthly_counters(pool, user_id).await?;
    }

    if is_new_week {
        reset_weekly_counters(pool, user_id).await?;
    }

    if is_new_day {
        record_sign_in_day(pool, user_id, now).await?;

        reset_daily_counters(pool, user_id).await?;
    }

    let ps_rows = sqlx::query(
        "UPDATE player_state
         SET last_sign_in_time = ?,
            updated_at = ?,
            initial_login_complete = 0,
            month_card_claimed = 0,
            last_month_card_claim_timestamp = NULL
         WHERE player_id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();

    if ps_rows == 0 {
        anyhow::bail!("player_state row missing for user_id={}", user_id);
    }

    Ok((is_new_day, is_new_week, is_new_month))
}

pub async fn process_manual_sign_in(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let now = ServerTime::now_ms();

    let recorded = record_sign_in_day(pool, user_id, now).await?;

    if !recorded {
        tracing::info!("User {} already signed in today", user_id);
    }

    Ok(recorded)
}

/// Reset daily counters (call this for any daily-reset systems)
pub async fn reset_daily_counters(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let reset_task_ids = tasks::reset_daily_tasks(pool, user_id).await?;

    // Reset dungeon daily attempts
    sqlx::query("UPDATE user_dungeons SET today_pass_num = 0 WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    // Reset chapter type daily nums
    sqlx::query("UPDATE user_chapter_type_nums SET today_pass_num = 0 WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    sqlx::query("UPDATE player_state SET power_buy_count = 0 WHERE player_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    // Reset hero touch count
    let touch_count = config::configs::get()
        .r#const
        .get(HERO_TOUCH_COUNT_CONFIG_ID)
        .and_then(|row| row.value.parse::<i32>().ok())
        .unwrap_or_default();
    sqlx::query(
        r#"
        INSERT INTO hero_touch_count (user_id, touch_count_left)
        VALUES (?, ?)
        ON CONFLICT(user_id) DO UPDATE SET touch_count_left = excluded.touch_count_left
        "#,
    )
    .bind(user_id)
    .bind(touch_count)
    .execute(pool)
    .await?;

    tracing::info!("Reset daily counters for user {}", user_id);
    Ok(reset_task_ids)
}

/// Reset weekly counters (call this for any weekly-reset systems)
pub async fn reset_weekly_counters(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let game_data = config::configs::get();

    let reset_task_ids = tasks::reset_weekly_tasks(pool, user_id).await?;

    let weekly_store_goods: Vec<i32> = game_data
        .store_goods
        .iter()
        .filter(|g| g.refresh_time == 2)
        .map(|g| g.id)
        .collect();

    if !weekly_store_goods.is_empty() {
        let mut reset_count = 0;
        for goods_id in &weekly_store_goods {
            reset_count += sqlx::query(
                "UPDATE user_store_goods
                 SET buy_count = 0
                 WHERE user_id = ? AND goods_id = ?",
            )
            .bind(user_id)
            .bind(goods_id)
            .execute(pool)
            .await?
            .rows_affected();
        }

        if reset_count > 0 {
            tracing::info!(
                "Reset weekly store_goods for user {}: {} items reset",
                user_id,
                reset_count
            );
        }
    }

    let weekly_charge_goods: Vec<i32> = game_data
        .store_charge_goods
        .iter()
        .filter(|g| g.order == 40)
        .map(|g| g.id)
        .collect();

    if !weekly_charge_goods.is_empty() {
        let mut reset_count = 0;
        for goods_id in &weekly_charge_goods {
            reset_count += sqlx::query(
                "UPDATE user_charge_info
                 SET buy_count = 0
                 WHERE user_id = ? AND charge_id = ?",
            )
            .bind(user_id)
            .bind(goods_id)
            .execute(pool)
            .await?
            .rows_affected();
        }

        if reset_count > 0 {
            tracing::info!(
                "Reset weekly store_charge_goods for user {}: {} items reset",
                user_id,
                reset_count
            );
        }
    }

    if weekly_store_goods.is_empty() && weekly_charge_goods.is_empty() {
        tracing::info!(
            "Reset weekly counters for user {} (no weekly items)",
            user_id
        );
    }

    Ok(reset_task_ids)
}

/// Reset monthly counters
pub async fn reset_monthly_counters(pool: &SqlitePool, user_id: i64) -> Result<()> {
    let game_data = config::configs::get();

    sqlx::query(
        "UPDATE user_sign_in_info
             SET addup_sign_in_day = 0
             WHERE user_id = ?",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    let monthly_store_goods: Vec<i32> = game_data
        .store_goods
        .iter()
        .filter(|g| g.refresh_time == 3)
        .map(|g| g.id)
        .collect();

    if !monthly_store_goods.is_empty() {
        let mut reset_count = 0;
        for goods_id in &monthly_store_goods {
            reset_count += sqlx::query(
                "UPDATE user_store_goods
                 SET buy_count = 0
                 WHERE user_id = ? AND goods_id = ?",
            )
            .bind(user_id)
            .bind(goods_id)
            .execute(pool)
            .await?
            .rows_affected();
        }

        if reset_count > 0 {
            tracing::info!(
                "Reset monthly store_goods for user {}: {} items reset",
                user_id,
                reset_count
            );
        }
    }

    let monthly_charge_goods: Vec<i32> = game_data
        .store_charge_goods
        .iter()
        .filter(|g| {
            g.belong_store_id == 614 && matches!(g.order, 120 | 311 | 312 | 350 | 500 | 600)
        })
        .map(|g| g.id)
        .collect();

    if !monthly_charge_goods.is_empty() {
        let mut reset_count = 0;
        for goods_id in &monthly_charge_goods {
            reset_count += sqlx::query(
                "UPDATE user_charge_info
                 SET buy_count = 0
                 WHERE user_id = ? AND charge_id = ?",
            )
            .bind(user_id)
            .bind(goods_id)
            .execute(pool)
            .await?
            .rows_affected();
        }

        if reset_count > 0 {
            tracing::info!(
                "Reset monthly store_charge_goods for user {}: {} items reset",
                user_id,
                reset_count
            );
        }
    }

    if monthly_store_goods.is_empty() && monthly_charge_goods.is_empty() {
        tracing::info!(
            "Reset monthly counters for user {} (no monthly items)",
            user_id
        );
    }

    Ok(())
}

pub async fn ensure_sign_in_info(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO user_sign_in_info (user_id, addup_sign_in_day, open_function_time, reward_mark)
        VALUES (?, 0, 0, 0)
        ON CONFLICT(user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn ensure_sign_in_info_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_sign_in_info
             (user_id, addup_sign_in_day, open_function_time, reward_mark)
         VALUES (?, 0, 0, 0)
         ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn addup_sign_in_days(pool: &SqlitePool, user_id: i64) -> Result<i32> {
    Ok(
        sqlx::query_scalar("SELECT addup_sign_in_day FROM user_sign_in_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_default(),
    )
}

pub async fn addup_sign_in_days_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<i32> {
    Ok(
        sqlx::query_scalar("SELECT addup_sign_in_day FROM user_sign_in_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .unwrap_or_default(),
    )
}

pub async fn claim_addup_bonus(pool: &SqlitePool, user_id: i64, bonus_id: i32) -> Result<bool> {
    Ok(sqlx::query(
        "INSERT OR IGNORE INTO user_sign_in_addup_bonus (user_id, bonus_id) VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(bonus_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn claim_addup_bonus_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    bonus_id: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "INSERT OR IGNORE INTO user_sign_in_addup_bonus (user_id, bonus_id) VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(bonus_id)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn lifetime_reward_state(pool: &SqlitePool, user_id: i64) -> Result<(i32, i32)> {
    Ok(sqlx::query_as(
        "SELECT player.total_login_days, sign_in.reward_mark
         FROM player_info AS player
         JOIN user_sign_in_info AS sign_in ON sign_in.user_id = player.player_id
         WHERE player.player_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

pub async fn lifetime_reward_state_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<(i32, i32)> {
    Ok(sqlx::query_as(
        "SELECT player.total_login_days, sign_in.reward_mark
         FROM player_info AS player
         JOIN user_sign_in_info AS sign_in ON sign_in.user_id = player.player_id
         WHERE player.player_id = ?",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn update_reward_mark(
    pool: &SqlitePool,
    user_id: i64,
    old_mark: i32,
    new_mark: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_sign_in_info
         SET reward_mark = ?
         WHERE user_id = ? AND reward_mark = ?",
    )
    .bind(new_mark)
    .bind(user_id)
    .bind(old_mark)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn update_reward_mark_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    old_mark: i32,
    new_mark: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_sign_in_info
         SET reward_mark = ?
         WHERE user_id = ? AND reward_mark = ?",
    )
    .bind(new_mark)
    .bind(user_id)
    .bind(old_mark)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn reward_mark(pool: &SqlitePool, user_id: i64) -> Result<i32> {
    Ok(
        sqlx::query_scalar("SELECT reward_mark FROM user_sign_in_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn get_sign_in_info(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(
    UserSignInInfo,
    Vec<i32>,              // sign-in days (day_of_month)
    Vec<i32>,              // addup bonus ids
    Vec<i32>,              // month card days (day_of_month)
    Vec<MonthCardHistory>, // month card history
    Vec<i32>,              // birthday heroes
)> {
    let info =
        sqlx::query_as::<_, UserSignInInfo>("SELECT * FROM user_sign_in_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(UserSignInInfo {
                user_id,
                addup_sign_in_day: 0,
                open_function_time: 0,
                reward_mark: 0,
            });

    let sign_in_days = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT day_of_month
        FROM user_sign_in_days
        WHERE user_id = ?
        ORDER BY server_day
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let addup_bonus = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT bonus_id
        FROM user_sign_in_addup_bonus
        WHERE user_id = ?
        ORDER BY bonus_id
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let month_card_days = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT day_of_month
        FROM user_month_card_days
        WHERE user_id = ?
        ORDER BY server_day
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let month_card_history = sqlx::query_as::<_, MonthCardHistory>(
        r#"
        SELECT card_id, start_time, end_time
        FROM user_month_card_history
        WHERE user_id = ?
        ORDER BY start_time
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let birthday_heroes = get_claimed_birthday_heroes_today(pool, user_id).await?;

    Ok((
        info,
        sign_in_days,
        addup_bonus,
        month_card_days,
        month_card_history,
        birthday_heroes,
    ))
}

pub async fn get_month_card_infos(pool: &SqlitePool, user_id: i64) -> Result<Vec<MonthCardInfo>> {
    let now_ms = ServerTime::now_ms();
    let server_day = ServerTime::server_day(now_ms);

    let active_cards = sqlx::query_as::<_, (i32, i32)>(
        r#"
        SELECT card_id, end_time
        FROM user_month_card_history
        WHERE user_id = ? AND end_time > ?
        ORDER BY card_id
        "#,
    )
    .bind(user_id)
    .bind((now_ms / 1000) as i32)
    .fetch_all(pool)
    .await?;

    let claimed_today = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT 1
        FROM user_month_card_days
        WHERE user_id = ? AND server_day = ?
        "#,
    )
    .bind(user_id)
    .bind(server_day)
    .fetch_optional(pool)
    .await?
    .is_some();

    Ok(active_cards
        .into_iter()
        .filter(|(card_id, _)| config::configs::get().month_card.get(*card_id).is_some())
        .map(|(card_id, end_time)| MonthCardInfo {
            id: Some(card_id),
            expire_time: Some(end_time),
            has_get_bonus: Some(claimed_today),
        })
        .collect())
}

pub async fn claim_month_card_bonus(pool: &SqlitePool, user_id: i64, card_id: i32) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let claimed = claim_month_card_bonus_in_transaction(&mut tx, user_id, card_id).await?;
    tx.commit().await?;
    Ok(claimed)
}

pub async fn claim_month_card_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    card_id: i32,
) -> Result<bool> {
    let now_ms = ServerTime::now_ms();
    let now_sec = (now_ms / 1000) as i32;
    let active = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT 1
        FROM user_month_card_history
        WHERE user_id = ? AND card_id = ? AND end_time > ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(card_id)
    .bind(now_sec)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if !active {
        return Ok(false);
    }

    let server_day = ServerTime::server_day(now_ms);
    let day_of_month = ServerTime::day_of_month(now_ms);
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_month_card_days (user_id, server_day, day_of_month)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(server_day)
    .bind(day_of_month)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn purchase_month_card_attachment(
    pool: &SqlitePool,
    user_id: i64,
    goods_id: i32,
    now_ms: i64,
) -> Result<Option<String>> {
    let mut tx = pool.begin().await?;
    let attachment =
        purchase_month_card_attachment_in_transaction(&mut tx, user_id, goods_id, now_ms).await?;
    tx.commit().await?;
    Ok(attachment)
}

pub async fn purchase_month_card_attachment_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    goods_id: i32,
    now_ms: i64,
) -> Result<Option<String>> {
    let Some(product) = month_card_product(goods_id) else {
        return Ok(None);
    };

    let now_sec = (now_ms / 1000) as i32;
    let latest = sqlx::query_as::<_, (i64, i32)>(
        r#"
        SELECT id, end_time
        FROM user_month_card_history
        WHERE user_id = ? AND card_id = ?
        ORDER BY end_time DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(product.card_id)
    .fetch_optional(&mut **tx)
    .await?;

    let base_time = latest
        .map(|(_, end_time)| end_time.max(now_sec))
        .unwrap_or(now_sec);
    let max_end_time = if product.max_days_limit <= 0 {
        i32::MAX
    } else {
        now_sec.saturating_add(product.max_days_limit.saturating_mul(DAY_SECONDS))
    };

    if base_time >= max_end_time {
        return Ok(Some(product.over_max_day_bonus));
    }

    let end_time = base_time
        .saturating_add(product.days.saturating_mul(DAY_SECONDS))
        .min(max_end_time);

    if let Some((id, _)) = latest {
        sqlx::query("UPDATE user_month_card_history SET end_time = ? WHERE id = ?")
            .bind(end_time)
            .bind(id)
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO user_month_card_history (user_id, card_id, start_time, end_time)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(product.card_id)
        .bind(now_sec)
        .bind(end_time)
        .execute(&mut **tx)
        .await?;
    }

    Ok(Some(product.once_bonus))
}

struct MonthCardProduct {
    card_id: i32,
    days: i32,
    max_days_limit: i32,
    once_bonus: String,
    over_max_day_bonus: String,
}

fn month_card_product(goods_id: i32) -> Option<MonthCardProduct> {
    let tables = config::configs::get();

    if let Some(card) = tables.month_card.get(goods_id) {
        return Some(MonthCardProduct {
            card_id: card.id,
            days: card.days,
            max_days_limit: card.max_days_limit,
            once_bonus: card.once_bonus.clone(),
            over_max_day_bonus: card.over_max_day_bonus.clone(),
        });
    }

    let added = tables.month_card_added.get(goods_id)?;
    let parent = tables.month_card.get(added.month_id)?;
    Some(MonthCardProduct {
        card_id: parent.id,
        days: added.days,
        max_days_limit: parent.max_days_limit,
        once_bonus: added.once_bonus.clone(),
        over_max_day_bonus: added.over_max_day_bonus.clone(),
    })
}

pub async fn add_sign_in_day(
    pool: &SqlitePool,
    user_id: i64,
    server_day: i32,
    day_of_month: i32,
    now_ms: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO user_sign_in_days (user_id, server_day, day_of_month)
        VALUES (?, ?, ?)
        ON CONFLICT(user_id, server_day) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(server_day)
    .bind(day_of_month)
    .execute(pool)
    .await?;

    let now_sec = (now_ms / 1000) as i32;

    sqlx::query(
        r#"
        INSERT INTO user_sign_in_info
            (user_id, addup_sign_in_day, open_function_time, reward_mark)
        VALUES (?, 1, ?, 0)
        ON CONFLICT(user_id)
        DO UPDATE SET addup_sign_in_day = addup_sign_in_day + 1
        "#,
    )
    .bind(user_id)
    .bind(now_sec)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get heroes whose birthday is today (using server time for consistency)
pub async fn get_claimed_birthday_heroes_today(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<i32>> {
    let server_now = common::time::ServerTime::server_date();
    let current_month = server_now.month();
    let current_day = server_now.day();
    let current_year = server_now.year();

    let game_data = config::get();

    let mut birthday_hero_ids = Vec::new();
    for character in game_data.character.iter() {
        if let Some((month_str, day_str)) = character.role_birthday.split_once('/')
            && let (Ok(month), Ok(day)) = (month_str.parse::<u32>(), day_str.parse::<u32>())
            && month == current_month
            && day == current_day
        {
            birthday_hero_ids.push(character.id);
        }
    }

    if birthday_hero_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = birthday_hero_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        "SELECT hero_id
         FROM hero_birthday_info
         WHERE user_id = ? AND last_claim_year = ? AND hero_id IN ({})",
        placeholders
    );

    let mut query = sqlx::query_scalar(&query).bind(user_id).bind(current_year);
    for hero_id in birthday_hero_ids {
        query = query.bind(hero_id);
    }

    let owned_birthday_heroes = query.fetch_all(pool).await?;

    Ok(owned_birthday_heroes)
}

pub async fn get_hero_birthday_claim(
    pool: &SqlitePool,
    user_id: i64,
    hero_id: i32,
) -> Result<Option<(i32, i32)>> {
    Ok(sqlx::query_as(
        "SELECT birthday_count, last_claim_year
         FROM hero_birthday_info
         WHERE user_id = ? AND hero_id = ?",
    )
    .bind(user_id)
    .bind(hero_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn claim_hero_birthday_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    hero_id: i32,
    expected_count: i32,
    year: i32,
) -> Result<bool> {
    let updated = sqlx::query(
        "INSERT INTO hero_birthday_info
             (user_id, hero_id, birthday_count, last_claim_year)
         VALUES (?, ?, 1, ?)
         ON CONFLICT(user_id, hero_id) DO UPDATE SET
             birthday_count = birthday_count + 1,
             last_claim_year = excluded.last_claim_year
         WHERE birthday_count = ? AND last_claim_year != excluded.last_claim_year",
    )
    .bind(user_id)
    .bind(hero_id)
    .bind(year)
    .bind(expected_count)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() == 1)
}
