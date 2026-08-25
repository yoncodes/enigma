use crate::db::game::hero_groups;
use crate::models::game::hero_group_snapshots::{
    HeroGroupSnapshot, HeroGroupSnapshotGroup, HeroGroupSnapshotInfo,
};
use crate::models::game::hero_groups::{HeroGroupEquip, HeroGroupInfo};
use anyhow::{Result, anyhow};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{BTreeMap, HashSet};

/// Helper to build HeroGroupInfo from a snapshot group
async fn build_snapshot_group_info(
    pool: &SqlitePool,
    snapshot_group_id: i64,
    group_id: i32,
) -> Result<HeroGroupInfo> {
    // Get group details
    let group = sqlx::query_as::<_, HeroGroupSnapshotGroup>(
        "SELECT * FROM hero_group_snapshot_groups WHERE id = ?",
    )
    .bind(snapshot_group_id)
    .fetch_one(pool)
    .await?;

    // Get hero members
    let hero_list: Vec<i64> = sqlx::query_scalar(
        "SELECT hero_uid FROM hero_group_snapshot_members WHERE snapshot_group_id = ? ORDER BY position"
    )
    .bind(snapshot_group_id)
    .fetch_all(pool)
    .await?;

    // Get equips
    let equip_rows: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT index_slot, equip_uid FROM hero_group_snapshot_equips WHERE snapshot_group_id = ? ORDER BY index_slot"
    )
    .bind(snapshot_group_id)
    .fetch_all(pool)
    .await?;

    let mut equips_map: BTreeMap<i32, Vec<i64>> = BTreeMap::new();
    for (index, equip_uid) in equip_rows {
        equips_map.entry(index).or_default().push(equip_uid);
    }

    let equips = equips_map
        .into_iter()
        .map(|(index, equip_uids)| HeroGroupEquip { index, equip_uids })
        .collect();

    // Get activity104 equips
    let activity104_rows: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT index_slot, equip_uid FROM hero_group_snapshot_activity104_equips WHERE snapshot_group_id = ? ORDER BY index_slot"
    )
    .bind(snapshot_group_id)
    .fetch_all(pool)
    .await?;

    let mut activity104_map: BTreeMap<i32, Vec<i64>> = BTreeMap::new();
    for (index, equip_uid) in activity104_rows {
        activity104_map.entry(index).or_default().push(equip_uid);
    }

    let activity104_equips = activity104_map
        .into_iter()
        .map(|(index, equip_uids)| HeroGroupEquip { index, equip_uids })
        .collect();

    tracing::info!(
        "Loaded group {}: sub {} {} heroes: {:?}",
        group_id,
        snapshot_group_id,
        hero_list.len(),
        hero_list
    );

    Ok(HeroGroupInfo {
        group_id,
        hero_list,
        name: group.name,
        cloth_id: group.cloth_id,
        equips,
        activity104_equips,
        assist_boss_id: group.assist_boss_id,
        params: group.params,
    })
}

/// Get all snapshots for a user
pub async fn get_hero_group_snapshots(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<HeroGroupSnapshotInfo>> {
    let snapshots = sqlx::query_as::<_, HeroGroupSnapshot>(
        "SELECT * FROM hero_group_snapshots WHERE user_id = ? ORDER BY snapshot_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();

    for snapshot in snapshots {
        // Get all groups in this snapshot
        let snapshot_groups = sqlx::query_as::<_, HeroGroupSnapshotGroup>(
            "SELECT * FROM hero_group_snapshot_groups WHERE snapshot_id = ? ORDER BY group_id",
        )
        .bind(snapshot.id)
        .fetch_all(pool)
        .await?;

        let mut hero_group_snapshots = Vec::new();
        for group in snapshot_groups {
            let info = build_snapshot_group_info(pool, group.id, group.group_id).await?;
            hero_group_snapshots.push(info);
        }

        // Get sort sub IDs
        let sort_sub_ids: Vec<i32> = sqlx::query_scalar(
            "SELECT sub_id FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ? ORDER BY sort_order"
        )
        .bind(snapshot.id)
        .fetch_all(pool)
        .await?;

        result.push(HeroGroupSnapshotInfo {
            snapshot_id: snapshot.snapshot_id,
            hero_group_snapshots,
            sort_sub_ids,
        });
    }

    Ok(result)
}

/// Get a specific snapshot by ID
pub async fn get_hero_group_snapshot(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
) -> Result<Option<HeroGroupSnapshotInfo>> {
    let snapshot = sqlx::query_as::<_, HeroGroupSnapshot>(
        "SELECT * FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .fetch_optional(pool)
    .await?;

    let Some(snapshot) = snapshot else {
        return Ok(None);
    };

    // Get all groups in this snapshot
    let snapshot_groups = sqlx::query_as::<_, HeroGroupSnapshotGroup>(
        "SELECT * FROM hero_group_snapshot_groups WHERE snapshot_id = ? ORDER BY group_id",
    )
    .bind(snapshot.id)
    .fetch_all(pool)
    .await?;

    let mut hero_group_snapshots = Vec::new();
    for group in snapshot_groups {
        let info = build_snapshot_group_info(pool, group.id, group.group_id).await?;
        hero_group_snapshots.push(info);
    }

    // Get sort sub IDs
    let sort_sub_ids: Vec<i32> = sqlx::query_scalar(
        "SELECT sub_id FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ? ORDER BY sort_order",
    )
    .bind(snapshot.id)
    .fetch_all(pool)
    .await?;

    Ok(Some(HeroGroupSnapshotInfo {
        snapshot_id: snapshot.snapshot_id,
        hero_group_snapshots,
        sort_sub_ids,
    }))
}

pub async fn rename_hero_group_snapshot(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
    group_id: i32,
    name: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE hero_group_snapshot_groups
         SET name = ?
         WHERE group_id = ? AND snapshot_id = (
             SELECT id FROM hero_group_snapshots
             WHERE user_id = ? AND snapshot_id = ?
         )",
    )
    .bind(name)
    .bind(group_id)
    .bind(user_id)
    .bind(snapshot_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}

pub async fn replace_hero_group_sort(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
    sort_sub_ids: &[i32],
    common_snapshot: bool,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    if common_snapshot {
        sqlx::query(
            "INSERT OR IGNORE INTO hero_group_snapshots
             (user_id, snapshot_id, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(snapshot_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    let Some(db_snapshot_id): Option<i64> = sqlx::query_scalar(
        "SELECT id FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    let mut group_ids: Vec<i32> =
        sqlx::query_scalar("SELECT group_id FROM hero_group_snapshot_groups WHERE snapshot_id = ?")
            .bind(db_snapshot_id)
            .fetch_all(&mut *tx)
            .await?;
    if group_ids.is_empty() && common_snapshot {
        group_ids = sqlx::query_scalar("SELECT group_id FROM hero_groups_common WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&mut *tx)
            .await?;
    }

    let requested = sort_sub_ids.iter().copied().collect::<HashSet<_>>();
    let existing = group_ids.into_iter().collect::<HashSet<_>>();
    if requested.len() != sort_sub_ids.len() || requested != existing {
        return Ok(false);
    }

    sqlx::query("DELETE FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ?")
        .bind(db_snapshot_id)
        .execute(&mut *tx)
        .await?;
    for (order, sub_id) in sort_sub_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO hero_group_snapshot_sort_ids (snapshot_id, sub_id, sort_order)
             VALUES (?, ?, ?)",
        )
        .bind(db_snapshot_id)
        .bind(sub_id)
        .bind(order as i32)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE hero_group_snapshots SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(db_snapshot_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn delete_hero_group(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
    snapshot_sub_id: i32,
    common_snapshot: bool,
) -> Result<Option<Vec<i32>>> {
    let mut tx = pool.begin().await?;
    let db_snapshot_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .fetch_optional(&mut *tx)
    .await?;

    let snapshot_deleted = if let Some(db_snapshot_id) = db_snapshot_id {
        sqlx::query(
            "DELETE FROM hero_group_snapshot_groups
             WHERE snapshot_id = ? AND group_id = ?",
        )
        .bind(db_snapshot_id)
        .bind(snapshot_sub_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1
    } else {
        false
    };
    let common_deleted = if common_snapshot {
        sqlx::query("DELETE FROM hero_groups_common WHERE user_id = ? AND group_id = ?")
            .bind(user_id)
            .bind(snapshot_sub_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1
    } else {
        false
    };
    if !snapshot_deleted && !common_deleted {
        return Ok(None);
    }

    let sort_sub_ids = if let Some(db_snapshot_id) = db_snapshot_id {
        sqlx::query(
            "DELETE FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ? AND sub_id = ?",
        )
        .bind(db_snapshot_id)
        .bind(snapshot_sub_id)
        .execute(&mut *tx)
        .await?;
        let ids: Vec<i32> = sqlx::query_scalar(
            "SELECT sub_id FROM hero_group_snapshot_sort_ids
             WHERE snapshot_id = ? ORDER BY sort_order",
        )
        .bind(db_snapshot_id)
        .fetch_all(&mut *tx)
        .await?;
        for (order, sub_id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE hero_group_snapshot_sort_ids SET sort_order = ?
                 WHERE snapshot_id = ? AND sub_id = ?",
            )
            .bind(order as i32)
            .bind(db_snapshot_id)
            .bind(sub_id)
            .execute(&mut *tx)
            .await?;
        }
        ids
    } else {
        sqlx::query_scalar(
            "SELECT group_id FROM hero_groups_common WHERE user_id = ? ORDER BY group_id",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?
    };

    if common_deleted {
        sqlx::query(
            "UPDATE hero_group_types
             SET current_select = COALESCE(
                 (SELECT MIN(group_id) FROM hero_groups_common WHERE user_id = ?), 0
             ), updated_at = ?
             WHERE user_id = ? AND current_select = ?",
        )
        .bind(user_id)
        .bind(common::time::ServerTime::now_ms())
        .bind(user_id)
        .bind(snapshot_sub_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(Some(sort_sub_ids))
}

/// Save a snapshot from current hero groups
pub async fn save_hero_group_snapshot(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
    groups: Vec<HeroGroupInfo>,
    sort_sub_ids: Vec<i32>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    save_hero_group_snapshot_in_transaction(&mut tx, user_id, snapshot_id, &groups, &sort_sub_ids)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn save_hero_group_snapshot_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    snapshot_id: i32,
    groups: &[HeroGroupInfo],
    sort_sub_ids: &[i32],
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();

    // Create or update snapshot
    sqlx::query(
        "INSERT INTO hero_group_snapshots (user_id, snapshot_id, created_at, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, snapshot_id) DO UPDATE SET updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    // Get the snapshot DB ID
    let db_snapshot_id: i64 = sqlx::query_scalar(
        "SELECT id FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .fetch_one(&mut **tx)
    .await?;

    for group in groups {
        let existing_group: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM hero_group_snapshot_groups
             WHERE snapshot_id = ? AND group_id = ?",
        )
        .bind(db_snapshot_id)
        .bind(group.group_id)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(old_group_id) = existing_group {
            replace_group_children(tx, old_group_id, group, true).await?;
            sqlx::query("DELETE FROM hero_group_snapshot_groups WHERE id = ?")
                .bind(old_group_id)
                .execute(&mut **tx)
                .await?;
        }

        let snapshot_group_id = sqlx::query(
            "INSERT INTO hero_group_snapshot_groups
             (snapshot_id, group_id, name, cloth_id, assist_boss_id, params)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(db_snapshot_id)
        .bind(group.group_id)
        .bind(&group.name)
        .bind(group.cloth_id)
        .bind(group.assist_boss_id)
        .bind(&group.params)
        .execute(&mut **tx)
        .await?
        .last_insert_rowid();
        insert_group_children(tx, snapshot_group_id, group, true).await?;
    }

    // Update sort IDs - merge with existing ones
    let existing_sort_ids: Vec<i32> = sqlx::query_scalar(
        "SELECT sub_id FROM hero_group_snapshot_sort_ids
         WHERE snapshot_id = ? ORDER BY sort_order",
    )
    .bind(db_snapshot_id)
    .fetch_all(&mut **tx)
    .await?;

    // Merge: add new sort_sub_ids if not already present
    let mut merged_sort_ids = existing_sort_ids;
    for sub_id in sort_sub_ids {
        if !merged_sort_ids.contains(sub_id) {
            merged_sort_ids.push(*sub_id);
        }
    }

    // Replace all sort IDs
    sqlx::query("DELETE FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ?")
        .bind(db_snapshot_id)
        .execute(&mut **tx)
        .await?;

    for (order, sub_id) in merged_sort_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO hero_group_snapshot_sort_ids (snapshot_id, sub_id, sort_order)
             VALUES (?, ?, ?)",
        )
        .bind(db_snapshot_id)
        .bind(sub_id)
        .bind(order as i32)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Replaces one common group and its matching snapshot as one authoritative write.
pub async fn save_common_group_snapshot_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    snapshot_id: i32,
    group: &HeroGroupInfo,
) -> Result<()> {
    if !hero_groups::group_assets_owned(tx, user_id, group).await? {
        return Err(anyhow!("hero group contains an unowned asset"));
    }

    let now = common::time::ServerTime::now_ms();
    let group_db_id: i64 = sqlx::query_scalar(
        "INSERT INTO hero_groups_common
             (user_id, group_id, name, cloth_id, assist_boss_id, params, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, group_id) DO UPDATE SET
             name = excluded.name,
             cloth_id = excluded.cloth_id,
             assist_boss_id = excluded.assist_boss_id,
             params = excluded.params,
             updated_at = excluded.updated_at
         RETURNING id",
    )
    .bind(user_id)
    .bind(group.group_id)
    .bind(&group.name)
    .bind(group.cloth_id)
    .bind(group.assist_boss_id)
    .bind(&group.params)
    .bind(now)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    replace_group_children(tx, group_db_id, group, false).await?;

    sqlx::query(
        "INSERT INTO hero_group_snapshots (user_id, snapshot_id, created_at, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, snapshot_id) DO UPDATE SET updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    let snapshot_db_id: i64 = sqlx::query_scalar(
        "SELECT id FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?",
    )
    .bind(user_id)
    .bind(snapshot_id)
    .fetch_one(&mut **tx)
    .await?;

    if let Some(old_group_id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM hero_group_snapshot_groups
         WHERE snapshot_id = ? AND group_id = ?",
    )
    .bind(snapshot_db_id)
    .bind(group.group_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        replace_group_children(tx, old_group_id, group, true).await?;
        sqlx::query("DELETE FROM hero_group_snapshot_groups WHERE id = ?")
            .bind(old_group_id)
            .execute(&mut **tx)
            .await?;
    }

    let snapshot_group_id = sqlx::query(
        "INSERT INTO hero_group_snapshot_groups
         (snapshot_id, group_id, name, cloth_id, assist_boss_id, params)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(snapshot_db_id)
    .bind(group.group_id)
    .bind(&group.name)
    .bind(group.cloth_id)
    .bind(group.assist_boss_id)
    .bind(&group.params)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid();
    insert_group_children(tx, snapshot_group_id, group, true).await?;

    sqlx::query(
        "INSERT OR IGNORE INTO hero_group_snapshot_sort_ids
         (snapshot_id, sub_id, sort_order)
         VALUES (?, ?, COALESCE((SELECT MAX(sort_order) + 1
             FROM hero_group_snapshot_sort_ids WHERE snapshot_id = ?), 0))",
    )
    .bind(snapshot_db_id)
    .bind(group.group_id)
    .bind(snapshot_db_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn rename_common_group_snapshot(
    pool: &SqlitePool,
    user_id: i64,
    snapshot_id: i32,
    group_id: i32,
    name: &str,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let common = sqlx::query(
        "UPDATE hero_groups_common SET name = ?, updated_at = ?
         WHERE user_id = ? AND group_id = ?",
    )
    .bind(name)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    let snapshot = sqlx::query(
        "UPDATE hero_group_snapshot_groups SET name = ?
         WHERE snapshot_id = (
            SELECT id FROM hero_group_snapshots WHERE user_id = ? AND snapshot_id = ?
         ) AND group_id = ?",
    )
    .bind(name)
    .bind(user_id)
    .bind(snapshot_id)
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    if common.rows_affected() == 0 && snapshot.rows_affected() == 0 {
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

async fn replace_group_children(
    tx: &mut Transaction<'_, Sqlite>,
    group_db_id: i64,
    group: &HeroGroupInfo,
    snapshot: bool,
) -> Result<()> {
    let (members, equips, activity_equips, key) = if snapshot {
        (
            "hero_group_snapshot_members",
            "hero_group_snapshot_equips",
            "hero_group_snapshot_activity104_equips",
            "snapshot_group_id",
        )
    } else {
        (
            "hero_group_members",
            "hero_group_equips",
            "hero_group_activity104_equips",
            "hero_group_id",
        )
    };
    for table in [members, equips, activity_equips] {
        sqlx::query(&format!("DELETE FROM {table} WHERE {key} = ?"))
            .bind(group_db_id)
            .execute(&mut **tx)
            .await?;
    }
    if !snapshot {
        insert_group_children(tx, group_db_id, group, false).await?;
    }
    Ok(())
}

async fn insert_group_children(
    tx: &mut Transaction<'_, Sqlite>,
    group_db_id: i64,
    group: &HeroGroupInfo,
    snapshot: bool,
) -> Result<()> {
    let prefix = if snapshot {
        "hero_group_snapshot"
    } else {
        "hero_group"
    };
    let key = if snapshot {
        "snapshot_group_id"
    } else {
        "hero_group_id"
    };
    for (position, hero_uid) in group.hero_list.iter().enumerate() {
        sqlx::query(&format!(
            "INSERT INTO {prefix}_members ({key}, hero_uid, position) VALUES (?, ?, ?)"
        ))
        .bind(group_db_id)
        .bind(hero_uid)
        .bind(position as i32)
        .execute(&mut **tx)
        .await?;
    }
    for (table, equips) in [
        (format!("{prefix}_equips"), &group.equips),
        (
            format!("{prefix}_activity104_equips"),
            &group.activity104_equips,
        ),
    ] {
        for equip in equips {
            for uid in &equip.equip_uids {
                sqlx::query(&format!(
                    "INSERT INTO {table} ({key}, index_slot, equip_uid) VALUES (?, ?, ?)"
                ))
                .bind(group_db_id)
                .bind(equip.index)
                .bind(uid)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(())
}

pub async fn sync_snapshot_to_common(
    pool: &SqlitePool,
    user_id: i64,
    group: &HeroGroupInfo,
) -> Result<()> {
    if super::hero_groups::update_hero_group(pool, user_id, group).await? {
        Ok(())
    } else {
        Err(anyhow!("hero group is not owned by user"))
    }
}
