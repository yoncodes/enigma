use crate::models::game::room_ob::{RoomFormula, RoomHero, RoomProductionLine, RoomSkin};
use anyhow::{Result, bail};
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

// Live schedules the next room-faith refresh ten minutes after a hero is placed.
const FAITH_REFRESH_SECONDS: i32 = 10 * 60;
const PRODUCT_LINE_LOGIC: i32 = 1;

pub async fn get_formulas(pool: &SqlitePool, user_id: i64) -> Result<Vec<RoomFormula>> {
    Ok(sqlx::query_as(
        "SELECT formula_id, count FROM user_room_formulas WHERE user_id = ? ORDER BY formula_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_production_lines(
    pool: &SqlitePool,
    user_id: i64,
    ids: &[i32],
) -> Result<Vec<RoomProductionLine>> {
    let mut lines = sqlx::query_as::<_, RoomProductionLine>(
        "SELECT line_id, formula_id, finish_count, next_finish_time, pause_time, level
         FROM user_room_production_lines
         WHERE user_id = ?
         ORDER BY line_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let now = ServerTime::now_sec_i32();
    for line in &mut lines {
        if refresh_production_line(line, now) {
            save_production_progress(pool, user_id, line).await?;
        }
    }

    if !ids.is_empty() {
        lines.retain(|line| ids.contains(&line.line_id));
    }
    lines.retain(|line| line.level > 0);

    Ok(lines)
}

pub async fn get_skins(pool: &SqlitePool, user_id: i64) -> Result<Vec<RoomSkin>> {
    Ok(sqlx::query_as(
        "SELECT part_id, skin_id FROM user_room_skins WHERE user_id = ? ORDER BY part_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn set_skin(
    pool: &SqlitePool,
    user_id: i64,
    part_id: i32,
    skin_id: i32,
) -> Result<RoomSkin> {
    sqlx::query(
        "INSERT INTO user_room_skins (user_id, part_id, skin_id)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, part_id) DO UPDATE SET skin_id = excluded.skin_id",
    )
    .bind(user_id)
    .bind(part_id)
    .bind(skin_id)
    .execute(pool)
    .await?;

    Ok(RoomSkin { part_id, skin_id })
}

pub async fn replace_skins(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    skins: &[sonettobuf::RoomSkinInfo],
) -> Result<()> {
    sqlx::query("DELETE FROM user_room_skins WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    for skin in skins {
        sqlx::query(
            "INSERT INTO user_room_skins (user_id, part_id, skin_id)
             VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(skin.id.unwrap_or_default())
        .bind(skin.skin_id.unwrap_or_default())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn get_heroes(
    pool: &SqlitePool,
    user_id: i64,
    hero_ids: &[i32],
) -> Result<Vec<RoomHero>> {
    let heroes: Vec<RoomHero> = sqlx::query_as(
        "SELECT hero_id, current_faith, next_refresh_time, skin, current_minute
         FROM user_room_heroes
         WHERE user_id = ?
         ORDER BY hero_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    if hero_ids.is_empty() {
        Ok(heroes)
    } else {
        let by_id = heroes
            .into_iter()
            .map(|hero| (hero.hero_id, hero))
            .collect::<HashMap<_, _>>();
        Ok(hero_ids
            .iter()
            .filter_map(|hero_id| by_id.get(hero_id).cloned())
            .collect())
    }
}

pub async fn full_faith_hero_ids(
    pool: &SqlitePool,
    user_id: i64,
    total_minutes: i32,
) -> Result<Vec<i32>> {
    Ok(sqlx::query_scalar(
        "SELECT hero_id FROM user_room_heroes
         WHERE user_id = ? AND current_minute >= ?
         ORDER BY hero_id",
    )
    .bind(user_id)
    .bind(total_minutes)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_heroes(
    pool: &SqlitePool,
    user_id: i64,
    hero_ids: &[i32],
) -> Result<Vec<RoomHero>> {
    let mut tx = pool.begin().await?;
    let next_refresh_time = ServerTime::now_sec_i32().saturating_add(FAITH_REFRESH_SECONDS);
    let current: Vec<i32> =
        sqlx::query_scalar("SELECT hero_id FROM user_room_heroes WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&mut *tx)
            .await?;

    for hero_id in current {
        if !hero_ids.contains(&hero_id) {
            sqlx::query("DELETE FROM user_room_heroes WHERE user_id = ? AND hero_id = ?")
                .bind(user_id)
                .bind(hero_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    for hero_id in hero_ids {
        sqlx::query(
            "INSERT INTO user_room_heroes (user_id, hero_id, skin, next_refresh_time)
             SELECT user_id, hero_id, skin, ? FROM heroes WHERE user_id = ? AND hero_id = ?
             ON CONFLICT DO NOTHING",
        )
        .bind(next_refresh_time)
        .bind(user_id)
        .bind(hero_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    get_heroes(pool, user_id, &[]).await
}

pub async fn gain_hero_faith(
    pool: &SqlitePool,
    user_id: i64,
    hero_ids: &[i32],
    max_faith: i32,
) -> Result<(Vec<RoomHero>, Vec<(i32, i32)>)> {
    if hero_ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut tx = pool.begin().await?;
    let next_refresh_time = ServerTime::now_sec_i32().saturating_add(FAITH_REFRESH_SECONDS);
    let mut changes = Vec::new();
    for hero_id in hero_ids {
        let (current_faith, faith): (i32, i32) = sqlx::query_as(
            "SELECT room.current_faith, hero.faith
             FROM user_room_heroes room
             JOIN heroes hero ON hero.user_id = room.user_id AND hero.hero_id = room.hero_id
             WHERE room.user_id = ? AND room.hero_id = ?",
        )
        .bind(user_id)
        .bind(hero_id)
        .fetch_one(&mut *tx)
        .await?;
        let gained = current_faith.min(max_faith.saturating_sub(faith)).max(0);

        if gained > 0 {
            sqlx::query("UPDATE heroes SET faith = faith + ? WHERE user_id = ? AND hero_id = ?")
                .bind(gained)
                .bind(user_id)
                .bind(hero_id)
                .execute(&mut *tx)
                .await?;
            changes.push((*hero_id, gained));
        }

        sqlx::query(
            "UPDATE user_room_heroes
             SET current_faith = 0,
                 current_minute = 0,
                 next_refresh_time = CASE
                     WHEN next_refresh_time > 0 THEN next_refresh_time
                     ELSE ?
                 END
             WHERE user_id = ? AND hero_id = ?",
        )
        .bind(next_refresh_time)
        .bind(user_id)
        .bind(hero_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok((get_heroes(pool, user_id, hero_ids).await?, changes))
}

pub async fn start_production_line(
    pool: &SqlitePool,
    user_id: i64,
    line_id: i32,
    formula_id: i32,
    requested_count: i32,
) -> Result<RoomProductionLine> {
    let level: i32 = sqlx::query_scalar(
        "SELECT level FROM user_room_production_lines WHERE user_id = ? AND line_id = ?",
    )
    .bind(user_id)
    .bind(line_id)
    .fetch_one(pool)
    .await?;
    let (finish_count, next_finish_time) = production_timing(line_id, formula_id, level)
        .map(|(_, cost_time)| (0, ServerTime::now_sec_i32().saturating_add(cost_time)))
        .unwrap_or((requested_count, 0));

    sqlx::query(
        "UPDATE user_room_production_lines
         SET formula_id = ?, finish_count = ?, next_finish_time = ?, pause_time = 0
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(formula_id)
    .bind(finish_count)
    .bind(next_finish_time)
    .bind(user_id)
    .bind(line_id)
    .execute(pool)
    .await?;

    get_production_line(pool, user_id, line_id).await
}

pub async fn start_production_line_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    line_id: i32,
    formula_id: i32,
    requested_count: i32,
) -> Result<RoomProductionLine> {
    let level: i32 = sqlx::query_scalar(
        "SELECT level FROM user_room_production_lines WHERE user_id = ? AND line_id = ?",
    )
    .bind(user_id)
    .bind(line_id)
    .fetch_one(&mut **tx)
    .await?;
    let (finish_count, next_finish_time) = production_timing(line_id, formula_id, level)
        .map(|(_, cost_time)| (0, ServerTime::now_sec_i32().saturating_add(cost_time)))
        .unwrap_or((requested_count, 0));

    sqlx::query_as(
        "UPDATE user_room_production_lines
         SET formula_id = ?, finish_count = ?, next_finish_time = ?, pause_time = 0
         WHERE user_id = ? AND line_id = ?
         RETURNING line_id, formula_id, finish_count, next_finish_time, pause_time, level",
    )
    .bind(formula_id)
    .bind(finish_count)
    .bind(next_finish_time)
    .bind(user_id)
    .bind(line_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

pub async fn gain_production_line(
    pool: &SqlitePool,
    user_id: i64,
    line_id: i32,
) -> Result<RoomProductionLine> {
    let current = get_production_line(pool, user_id, line_id).await?;
    let next_finish_time = production_timing(line_id, current.formula_id, current.level)
        .map(|(_, cost_time)| ServerTime::now_sec_i32().saturating_add(cost_time))
        .unwrap_or_default();

    sqlx::query(
        "UPDATE user_room_production_lines
         SET finish_count = 0, next_finish_time = ?, pause_time = 0
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(next_finish_time)
    .bind(user_id)
    .bind(line_id)
    .execute(pool)
    .await?;

    get_production_line(pool, user_id, line_id).await
}

pub async fn gain_production_lines_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    lines: &[RoomProductionLine],
) -> Result<Option<Vec<RoomProductionLine>>> {
    let mut changed = Vec::new();
    for line in lines {
        let next_finish_time = production_timing(line.line_id, line.formula_id, line.level)
            .map(|(_, cost_time)| ServerTime::now_sec_i32().saturating_add(cost_time))
            .unwrap_or_default();
        let updated = sqlx::query_as(
            "UPDATE user_room_production_lines
             SET finish_count = 0, next_finish_time = ?, pause_time = 0
             WHERE user_id = ? AND line_id = ? AND formula_id = ?
               AND finish_count = ? AND next_finish_time = ? AND level = ?
             RETURNING line_id, formula_id, finish_count, next_finish_time, pause_time, level",
        )
        .bind(next_finish_time)
        .bind(user_id)
        .bind(line.line_id)
        .bind(line.formula_id)
        .bind(line.finish_count)
        .bind(line.next_finish_time)
        .bind(line.level)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(updated) = updated else {
            return Ok(None);
        };
        changed.push(updated);
    }
    Ok(Some(changed))
}

pub async fn set_production_line_level(
    pool: &SqlitePool,
    user_id: i64,
    line_id: i32,
    level: i32,
) -> Result<RoomProductionLine> {
    sqlx::query(
        "UPDATE user_room_production_lines
         SET level = ?
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(level)
    .bind(user_id)
    .bind(line_id)
    .execute(pool)
    .await?;

    get_production_line(pool, user_id, line_id).await
}

pub async fn set_production_line_level_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    line_id: i32,
    current_level: i32,
    level: i32,
) -> Result<Option<RoomProductionLine>> {
    Ok(sqlx::query_as(
        "UPDATE user_room_production_lines
         SET level = ?
         WHERE user_id = ? AND line_id = ? AND level = ?
         RETURNING line_id, formula_id, finish_count, next_finish_time, pause_time, level",
    )
    .bind(level)
    .bind(user_id)
    .bind(line_id)
    .bind(current_level)
    .fetch_optional(&mut **tx)
    .await?)
}

pub async fn accelerate_production_line(
    pool: &SqlitePool,
    user_id: i64,
    line_id: i32,
) -> Result<RoomProductionLine> {
    sqlx::query(
        "UPDATE user_room_production_lines
         SET next_finish_time = 0, pause_time = 0
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(user_id)
    .bind(line_id)
    .execute(pool)
    .await?;

    get_production_line(pool, user_id, line_id).await
}

pub async fn unlock_production_lines(
    pool: &SqlitePool,
    user_id: i64,
    room_level: i32,
) -> Result<Vec<RoomProductionLine>> {
    for line in config::configs::get()
        .production_line
        .iter()
        .filter(|line| line.need_room_level <= room_level)
    {
        sqlx::query(
            "UPDATE user_room_production_lines
             SET level = 1
             WHERE user_id = ? AND line_id = ? AND level = 0",
        )
        .bind(user_id)
        .bind(line.id)
        .execute(pool)
        .await?;
    }

    get_production_lines(pool, user_id, &[]).await
}

async fn get_production_line(
    pool: &SqlitePool,
    user_id: i64,
    line_id: i32,
) -> Result<RoomProductionLine> {
    let Some(line) = sqlx::query_as(
        "SELECT line_id, formula_id, finish_count, next_finish_time, pause_time, level
         FROM user_room_production_lines
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(user_id)
    .bind(line_id)
    .fetch_optional(pool)
    .await?
    else {
        bail!("room production line {line_id} not found");
    };

    Ok(line)
}

async fn save_production_progress(
    pool: &SqlitePool,
    user_id: i64,
    line: &RoomProductionLine,
) -> Result<()> {
    sqlx::query(
        "UPDATE user_room_production_lines
         SET finish_count = ?, next_finish_time = ?, pause_time = ?
         WHERE user_id = ? AND line_id = ?",
    )
    .bind(line.finish_count)
    .bind(line.next_finish_time)
    .bind(line.pause_time)
    .bind(user_id)
    .bind(line.line_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn refresh_production_line(line: &mut RoomProductionLine, now: i32) -> bool {
    let Some((capacity, cost_time)) = production_timing(line.line_id, line.formula_id, line.level)
    else {
        return false;
    };

    if line.finish_count >= capacity {
        if line.pause_time == 0 {
            line.pause_time = line.next_finish_time.saturating_sub(cost_time);
            return true;
        }
        return false;
    }

    if line.next_finish_time <= 0 {
        line.next_finish_time = now.saturating_add(cost_time);
        return true;
    }
    if now < line.next_finish_time {
        return false;
    }

    let elapsed_cycles = 1 + (now - line.next_finish_time) / cost_time;
    let completed = elapsed_cycles.min(capacity - line.finish_count);
    line.finish_count += completed;
    line.next_finish_time = line
        .next_finish_time
        .saturating_add(completed.saturating_mul(cost_time));
    if line.finish_count == capacity {
        line.pause_time = line.next_finish_time.saturating_sub(cost_time);
    }
    true
}

fn production_timing(line_id: i32, formula_id: i32, level: i32) -> Option<(i32, i32)> {
    let game = config::configs::get();
    let line = game.production_line.get(line_id)?;
    if line.logic != PRODUCT_LINE_LOGIC || level <= 0 {
        return None;
    }
    let formula = game.formula.get(formula_id)?;
    if formula.cost_reserve <= 0 || formula.cost_time <= 0 {
        return None;
    }

    let mut reserve = line.reserve;
    let mut time_reduction: i32 = 0;
    if let Some(level) = game
        .production_line_level
        .by_group(line.level_group)
        .find(|row| row.id == level)
    {
        for effect in level.effect.split('|') {
            let mut fields = effect
                .split('#')
                .filter_map(|value| value.parse::<i32>().ok());
            match (fields.next(), fields.next()) {
                (Some(1), Some(value)) => reserve = reserve.saturating_add(value),
                (Some(2), Some(value)) => time_reduction = time_reduction.saturating_add(value),
                _ => {}
            }
        }
    }

    let capacity = reserve / formula.cost_reserve;
    let cost_time = formula
        .cost_time
        .saturating_mul((1000 - time_reduction).max(0))
        / 1000;
    (capacity > 0 && cost_time > 0).then_some((capacity, cost_time))
}

pub fn production_line_is_full(line: &RoomProductionLine) -> bool {
    production_timing(line.line_id, line.formula_id, line.level)
        .is_some_and(|(capacity, _)| line.finish_count >= capacity)
}
