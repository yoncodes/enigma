use anyhow::Result;
use sonettobuf::FightEquipRecord;
use sqlx::{Sqlite, SqlitePool, Transaction};

pub use crate::models::game::equipment::Equipment;

async fn next_equipment_uid(tx: &mut Transaction<'_, Sqlite>) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT COALESCE(MAX(uid), 29999999) + 1 FROM equipment")
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// Get all equipment for a user
pub async fn get_user_equipment(pool: &SqlitePool, user_id: i64) -> Result<Vec<Equipment>> {
    let equipment = sqlx::query_as::<_, Equipment>(
        "SELECT * FROM equipment WHERE user_id = ?1 AND count > 0 ORDER BY equip_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(equipment)
}

pub async fn get_hero_default_equip_id(
    pool: &SqlitePool,
    hero_uid: i64,
    user_id: i64,
) -> Result<Option<i32>> {
    let equip_id: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT e.equip_id
        FROM heroes h
        LEFT JOIN equipment e
          ON e.uid = h.default_equip_uid
        WHERE h.uid = ? AND h.user_id = ?
        "#,
    )
    .bind(hero_uid)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(equip_id)
}

pub async fn get_equipment_by_uid(
    pool: &SqlitePool,
    user_id: i64,
    equip_uid: i64,
) -> Result<Equipment> {
    let equip = sqlx::query_as::<_, Equipment>(
        r#"
        SELECT uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv, created_at, updated_at
        FROM equipment
        WHERE uid = ? AND user_id = ?
        "#,
    )
    .bind(equip_uid)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(equip)
}

pub struct StrengthenConsume {
    pub uid: i64,
    pub count: i32,
    pub expected_count: i32,
    pub stackable: bool,
}

pub struct StrengthenUpdate<'a> {
    pub target_uid: i64,
    pub expected_level: i32,
    pub expected_exp: i32,
    pub level: i32,
    pub exp: i32,
    pub consumes: &'a [StrengthenConsume],
}

pub struct RefineConsume {
    pub uid: i64,
    pub equip_id: i32,
    pub refine_level: i32,
}

pub async fn apply_strengthen_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    update: StrengthenUpdate<'_>,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let target = sqlx::query(
        "UPDATE equipment SET level = ?, exp = ?, updated_at = ?
         WHERE user_id = ? AND uid = ? AND level = ? AND exp = ? AND count > 0",
    )
    .bind(update.level)
    .bind(update.exp)
    .bind(now)
    .bind(user_id)
    .bind(update.target_uid)
    .bind(update.expected_level)
    .bind(update.expected_exp)
    .execute(&mut **tx)
    .await?;
    if target.rows_affected() != 1 {
        return Ok(false);
    }

    for consume in update.consumes {
        let remaining = consume.expected_count - consume.count;
        let result = if remaining > 0 || consume.stackable {
            sqlx::query(
                "UPDATE equipment SET count = ?, updated_at = ?
                 WHERE user_id = ? AND uid = ? AND count = ? AND is_lock = 0",
            )
            .bind(remaining.max(0))
            .bind(now)
            .bind(user_id)
            .bind(consume.uid)
            .bind(consume.expected_count)
            .execute(&mut **tx)
            .await?
        } else {
            sqlx::query(
                "DELETE FROM equipment
                 WHERE user_id = ? AND uid = ? AND count = ? AND is_lock = 0",
            )
            .bind(user_id)
            .bind(consume.uid)
            .bind(consume.expected_count)
            .execute(&mut **tx)
            .await?
        };
        if result.rows_affected() != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

pub async fn consume_item_and_max_equipment(
    pool: &SqlitePool,
    user_id: i64,
    item_id: u32,
    equipment_uid: i64,
    max_level: i32,
    max_break: i32,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    let consumed = sqlx::query(
        "UPDATE items
         SET quantity = quantity - 1, last_use_time = ?, last_update_time = ?
         WHERE user_id = ? AND item_id = ? AND quantity >= 1",
    )
    .bind(now)
    .bind(now)
    .bind(user_id)
    .bind(item_id)
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() != 1 {
        return Ok(false);
    }
    let updated = sqlx::query(
        "UPDATE equipment
         SET level = ?, exp = 0, break_lv = ?, updated_at = ?
         WHERE user_id = ? AND uid = ? AND count > 0",
    )
    .bind(max_level)
    .bind(max_break)
    .bind(now)
    .bind(user_id)
    .bind(equipment_uid)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

pub async fn update_equipment_lock(
    pool: &SqlitePool,
    user_id: i64,
    uid: i64,
    is_lock: bool,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();

    let rows_affected = sqlx::query(
        "UPDATE equipment SET is_lock = ?, updated_at = ? WHERE uid = ? AND user_id = ?",
    )
    .bind(is_lock)
    .bind(now)
    .bind(uid)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(rows_affected > 0)
}

pub async fn build_equip_records(
    pool: &SqlitePool,
    player_id: i64,
    fight_group: &Option<sonettobuf::FightGroup>,
) -> Result<Vec<FightEquipRecord>> {
    let Some(fg) = fight_group else {
        return Ok(vec![]);
    };

    let mut equip_records = Vec::new();

    for equip in &fg.equips {
        let hero_uid = equip.hero_uid.unwrap_or(0);
        let mut records = Vec::new();

        for &equip_uid in &equip.equip_uid {
            if equip_uid == 0 {
                continue;
            }

            if let Ok(equip_data) = get_equipment_by_uid(pool, player_id, equip_uid).await {
                records.push(sonettobuf::EquipRecord {
                    equip_uid: Some(equip_uid),
                    equip_id: Some(equip_data.equip_id),
                    equip_lv: Some(equip_data.level),
                    refine_lv: Some(equip_data.refine_lv),
                });
            }
        }

        equip_records.push(FightEquipRecord {
            hero_uid: Some(hero_uid),
            equip_records: records,
        });
    }

    Ok(equip_records)
}

pub async fn add_equipment(
    pool: &SqlitePool,
    user_id: i64,
    equip_id: i32,
    count: i32,
) -> Result<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let uids = add_equipment_in_transaction(&mut tx, user_id, equip_id, count).await?;
    tx.commit().await?;
    Ok(uids)
}

pub async fn add_equipment_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    equip_id: i32,
    count: i32,
) -> Result<Vec<i64>> {
    let now = common::time::ServerTime::now_ms();
    let game_data = config::configs::get();
    let equip = game_data
        .equip
        .get(equip_id)
        .ok_or_else(|| anyhow::anyhow!("Equipment {} not found", equip_id))?;

    let level = 1;
    let break_lv = 0;
    let refine_lv = 1;
    let is_lock = equip.rare >= 4 && equip.is_exp_equip == 0 && equip.is_sp_refine == 0;

    let is_stackable = equip.is_exp_equip == 1;

    let mut uids = Vec::new();

    if is_stackable {
        // Try to find existing stack
        if let Some(uid) = sqlx::query_scalar::<_, i64>(
            "SELECT uid FROM equipment WHERE user_id = ? AND equip_id = ? LIMIT 1",
        )
        .bind(user_id)
        .bind(equip_id)
        .fetch_optional(&mut **tx)
        .await?
        {
            sqlx::query(
                r#"
                UPDATE equipment
                SET count = count + ?, updated_at = ?
                WHERE uid = ? AND user_id = ?
                "#,
            )
            .bind(count)
            .bind(now)
            .bind(uid)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;

            uids.push(uid);
        } else {
            let uid = next_equipment_uid(tx).await?;

            sqlx::query(
                r#"
                INSERT INTO equipment
                  (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv, created_at, updated_at)
                VALUES
                  (?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(uid)
            .bind(user_id)
            .bind(equip_id)
            .bind(level)
            .bind(break_lv)
            .bind(count)
            .bind(is_lock)
            .bind(refine_lv)
            .bind(now)
            .bind(now)
            .execute(&mut **tx)
            .await?;

            uids.push(uid);
        }
    } else {
        let mut next_uid = next_equipment_uid(tx).await?;

        for _ in 0..count {
            sqlx::query(
                r#"
                INSERT INTO equipment
                  (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv, created_at, updated_at)
                VALUES
                  (?, ?, ?, ?, 0, ?, 1, ?, ?, ?, ?)
                "#,
            )
            .bind(next_uid)
            .bind(user_id)
            .bind(equip_id)
            .bind(level)
            .bind(break_lv)
            .bind(is_lock)
            .bind(refine_lv)
            .bind(now)
            .bind(now)
            .execute(&mut **tx)
            .await?;

            uids.push(next_uid);
            next_uid += 1;
        }
    }

    Ok(uids)
}

pub async fn advance_break_level_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    equip_uid: i64,
    expected_level: i32,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE equipment SET break_lv = break_lv + 1, updated_at = ?
         WHERE user_id = ? AND uid = ? AND break_lv = ?",
    )
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(equip_uid)
    .bind(expected_level)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn refine_equipment(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    target_uid: i64,
    expected_level: i32,
    level: i32,
    consumes: &[RefineConsume],
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE equipment SET refine_lv = ?, updated_at = ?
         WHERE user_id = ? AND uid = ? AND refine_lv = ?",
    )
    .bind(level)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(target_uid)
    .bind(expected_level)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(false);
    }

    for consume in consumes {
        let deleted = sqlx::query(
            "DELETE FROM equipment
             WHERE user_id = ? AND uid = ? AND uid != ? AND equip_id = ?
               AND refine_lv = ? AND count > 0 AND is_lock = 0",
        )
        .bind(user_id)
        .bind(consume.uid)
        .bind(target_uid)
        .bind(consume.equip_id)
        .bind(consume.refine_level)
        .execute(&mut **tx)
        .await?;
        if deleted.rows_affected() != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Get total count of equipment by equip_id (counts all matching rows)
pub async fn get_equipment_count(pool: &SqlitePool, user_id: i64, equip_id: i32) -> Result<i32> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM equipment WHERE user_id = ? AND equip_id = ?")
            .bind(user_id)
            .bind(equip_id)
            .fetch_one(pool)
            .await?;

    Ok(count as i32)
}

pub async fn update_equipment_count(
    pool: &SqlitePool,
    user_id: i64,
    equip_id: i32,
    amount: i32,
) -> Result<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let uids = update_equipment_count_in_transaction(&mut tx, user_id, equip_id, amount).await?;
    tx.commit().await?;
    Ok(uids)
}

pub async fn update_equipment_count_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    equip_id: i32,
    amount: i32,
) -> Result<Vec<i64>> {
    let now = common::time::ServerTime::now_ms();

    let uid = sqlx::query_scalar::<_, i64>(
        "SELECT uid FROM equipment WHERE user_id = ? AND equip_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(equip_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE equipment
        SET count = count + ?, updated_at = ?
        WHERE uid = ? AND user_id = ?
        "#,
    )
    .bind(amount)
    .bind(now)
    .bind(uid)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(vec![uid])
}

pub async fn add_equipments(
    pool: &SqlitePool,
    user_id: i64,
    equips: &[(i32, i32)],
) -> Result<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let uids = add_equipments_in_transaction(&mut tx, user_id, equips).await?;
    tx.commit().await?;
    Ok(uids)
}

pub async fn add_equipments_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    equips: &[(i32, i32)],
) -> Result<Vec<i64>> {
    let mut changed_uids = Vec::new();
    let game_data = config::configs::get();

    for (equip_id, count) in equips {
        let is_stackable = game_data
            .equip
            .get(*equip_id)
            .is_some_and(|equip| equip.is_exp_equip == 1);

        if is_stackable {
            if let Some(uid) = sqlx::query_scalar::<_, i64>(
                "SELECT uid FROM equipment WHERE user_id = ? AND equip_id = ? LIMIT 1",
            )
            .bind(user_id)
            .bind(equip_id)
            .fetch_optional(&mut **tx)
            .await?
            {
                update_equipment_count_in_transaction(tx, user_id, *equip_id, *count).await?;
                changed_uids.push(uid);
            } else {
                let uids = add_equipment_in_transaction(tx, user_id, *equip_id, *count).await?;
                debug_assert_eq!(uids.len(), 1);
                changed_uids.push(uids[0]);
            }
        } else {
            let uids = add_equipment_in_transaction(tx, user_id, *equip_id, *count).await?;
            changed_uids.extend(uids);
        }
    }

    Ok(changed_uids)
}

pub async fn decompose_equipment(
    pool: &SqlitePool,
    user_id: i64,
    equip_uids: &[i64],
    output_equip_id: i32,
    output_count: i32,
) -> Result<Vec<i64>> {
    let output = config::configs::get()
        .equip
        .get(output_equip_id)
        .ok_or_else(|| anyhow::anyhow!("Equipment {output_equip_id} not found"))?;
    if output.is_exp_equip != 1 || output_count <= 0 {
        anyhow::bail!("invalid equipment decomposition output");
    }

    let now = common::time::ServerTime::now_ms();
    let mut transaction = pool.begin().await?;
    for uid in equip_uids {
        let deleted = sqlx::query(
            "DELETE FROM equipment \
             WHERE uid = ? AND user_id = ? AND level = 1 AND count = 1 AND is_lock = 0",
        )
        .bind(uid)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if deleted.rows_affected() != 1 {
            transaction.rollback().await?;
            anyhow::bail!("equipment {uid} cannot be decomposed");
        }
    }

    let changed_uid = if let Some(uid) = sqlx::query_scalar::<_, i64>(
        "SELECT uid FROM equipment WHERE user_id = ? AND equip_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(output_equip_id)
    .fetch_optional(&mut *transaction)
    .await?
    {
        sqlx::query(
            "UPDATE equipment SET count = count + ?, updated_at = ? \
             WHERE uid = ? AND user_id = ?",
        )
        .bind(output_count)
        .bind(now)
        .bind(uid)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        uid
    } else {
        let uid = next_equipment_uid(&mut transaction).await?;
        sqlx::query(
            "INSERT INTO equipment \
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv, created_at, updated_at) \
             VALUES (?, ?, ?, 1, 0, 0, ?, 0, 1, ?, ?)",
        )
        .bind(uid)
        .bind(user_id)
        .bind(output_equip_id)
        .bind(output_count)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        uid
    };

    transaction.commit().await?;
    Ok(vec![changed_uid])
}
