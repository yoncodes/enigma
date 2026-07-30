use crate::models::game::hero_groups::{
    HeroGroupCommon, HeroGroupEquip, HeroGroupInfo, HeroGroupType, HeroGroupTypeInfo,
};
use anyhow::Result;
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeMap;

/// Helper to build HeroGroupInfo from a group_id
async fn build_hero_group_info(
    pool: &SqlitePool,
    _user_id: i64,
    db_group_id: i64,
    group_id: i32,
) -> Result<HeroGroupInfo> {
    // Get hero members
    let hero_list: Vec<i64> = sqlx::query_scalar(
        "SELECT hero_uid FROM hero_group_members WHERE hero_group_id = ? ORDER BY position",
    )
    .bind(db_group_id)
    .fetch_all(pool)
    .await?;

    // Get group details
    let group =
        sqlx::query_as::<_, HeroGroupCommon>("SELECT * FROM hero_groups_common WHERE id = ?")
            .bind(db_group_id)
            .fetch_one(pool)
            .await?;

    // Get equips
    let equip_rows: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT index_slot, equip_uid FROM hero_group_equips WHERE hero_group_id = ? ORDER BY index_slot"
    )
    .bind(db_group_id)
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
        "SELECT index_slot, equip_uid FROM hero_group_activity104_equips WHERE hero_group_id = ? ORDER BY index_slot"
    )
    .bind(db_group_id)
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

/// Get ONE specific hero group (for GetHeroGroupList - returns current active group)
pub async fn get_hero_group(
    pool: &SqlitePool,
    user_id: i64,
    group_id: i32,
) -> Result<Option<HeroGroupInfo>> {
    // Find the DB id for this group_id
    let db_group_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM hero_groups_common WHERE user_id = ? AND group_id = ?")
            .bind(user_id)
            .bind(group_id)
            .fetch_optional(pool)
            .await?;

    if let Some(db_id) = db_group_id {
        Ok(Some(
            build_hero_group_info(pool, user_id, db_id, group_id).await?,
        ))
    } else {
        Ok(None)
    }
}

/// Get current active group (probably type 1's current selection)
pub async fn get_current_hero_group(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Option<HeroGroupInfo>> {
    // Get the current selected group from type 1 (main battle group)
    let selected_group: Option<i32> = sqlx::query_scalar(
        "SELECT current_select FROM hero_group_types WHERE user_id = ? AND type_id = 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(group_id) = selected_group {
        get_hero_group(pool, user_id, group_id).await
    } else {
        Ok(None)
    }
}

/// Get ALL common hero groups (for GetHeroGroupCommonList)
pub async fn get_hero_groups_common(pool: &SqlitePool, user_id: i64) -> Result<Vec<HeroGroupInfo>> {
    let groups = sqlx::query_as::<_, HeroGroupCommon>(
        "SELECT * FROM hero_groups_common WHERE user_id = ? ORDER BY group_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for group in groups {
        let info = build_hero_group_info(pool, user_id, group.id, group.group_id).await?;
        result.push(info);
    }

    Ok(result)
}

/// Get all hero group types (for GetHeroGroupCommonList)
pub async fn get_hero_group_types(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<HeroGroupTypeInfo>> {
    let types = sqlx::query_as::<_, HeroGroupType>(
        "SELECT * FROM hero_group_types WHERE user_id = ? ORDER BY type_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for type_info in types {
        let group_info = if let Some(group_id) = type_info.group_id {
            get_hero_group(pool, user_id, group_id).await?
        } else {
            None
        };

        result.push(HeroGroupTypeInfo {
            type_id: type_info.type_id,
            current_select: type_info.current_select,
            group_info,
        });
    }

    Ok(result)
}

pub async fn set_current_selection(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    current_select: i32,
) -> Result<()> {
    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO hero_group_types
            (user_id, type_id, current_select, group_id, created_at, updated_at)
         VALUES (?, ?, ?, NULL, ?, ?)
         ON CONFLICT(user_id, type_id) DO UPDATE SET
            current_select = excluded.current_select,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(type_id)
    .bind(current_select)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn rename_hero_group(
    pool: &SqlitePool,
    user_id: i64,
    group_id: i32,
    name: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE hero_groups_common
         SET name = ?, updated_at = ?
         WHERE user_id = ? AND group_id = ?",
    )
    .bind(name)
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(group_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}

/// Replaces one owned common group as a single transaction.
pub async fn update_hero_group(
    pool: &SqlitePool,
    user_id: i64,
    group: &HeroGroupInfo,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let Some(db_group_id): Option<i64> =
        sqlx::query_scalar("SELECT id FROM hero_groups_common WHERE user_id = ? AND group_id = ?")
            .bind(user_id)
            .bind(group.group_id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(false);
    };

    if !group_assets_owned(&mut tx, user_id, group).await? {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE hero_groups_common
         SET name = ?, cloth_id = ?, assist_boss_id = ?, params = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&group.name)
    .bind(group.cloth_id)
    .bind(group.assist_boss_id)
    .bind(&group.params)
    .bind(ServerTime::now_ms())
    .bind(db_group_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM hero_group_members WHERE hero_group_id = ?")
        .bind(db_group_id)
        .execute(&mut *tx)
        .await?;
    for (position, hero_uid) in group.hero_list.iter().enumerate() {
        sqlx::query(
            "INSERT INTO hero_group_members (hero_group_id, hero_uid, position) VALUES (?, ?, ?)",
        )
        .bind(db_group_id)
        .bind(hero_uid)
        .bind(position as i32)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM hero_group_equips WHERE hero_group_id = ?")
        .bind(db_group_id)
        .execute(&mut *tx)
        .await?;
    for equip in &group.equips {
        for equip_uid in &equip.equip_uids {
            sqlx::query(
                "INSERT INTO hero_group_equips (hero_group_id, index_slot, equip_uid)
                 VALUES (?, ?, ?)",
            )
            .bind(db_group_id)
            .bind(equip.index)
            .bind(equip_uid)
            .execute(&mut *tx)
            .await?;
        }
    }

    sqlx::query("DELETE FROM hero_group_activity104_equips WHERE hero_group_id = ?")
        .bind(db_group_id)
        .execute(&mut *tx)
        .await?;
    for equip in &group.activity104_equips {
        for equip_uid in &equip.equip_uids {
            sqlx::query(
                "INSERT INTO hero_group_activity104_equips
                 (hero_group_id, index_slot, equip_uid) VALUES (?, ?, ?)",
            )
            .bind(db_group_id)
            .bind(equip.index)
            .bind(equip_uid)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(true)
}

pub(crate) async fn group_assets_owned(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    group: &HeroGroupInfo,
) -> Result<bool> {
    if group.cloth_id != 0
        && !sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_cloths WHERE user_id = ? AND cloth_id = ?)",
        )
        .bind(user_id)
        .bind(group.cloth_id)
        .fetch_one(&mut **tx)
        .await?
    {
        return Ok(false);
    }
    for hero_uid in group.hero_list.iter().copied().filter(|uid| *uid != 0) {
        if !sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM heroes WHERE user_id = ? AND uid = ?)")
            .bind(user_id)
            .bind(hero_uid)
            .fetch_one(&mut **tx)
            .await?
        {
            return Ok(false);
        }
    }
    for equip_uid in group
        .equips
        .iter()
        .chain(&group.activity104_equips)
        .flat_map(|equip| equip.equip_uids.iter().copied())
        .filter(|uid| *uid != 0)
    {
        if !sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM equipment WHERE user_id = ? AND uid = ?)",
        )
        .bind(user_id)
        .bind(equip_uid)
        .fetch_one(&mut **tx)
        .await?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Set equipment for a hero group
pub async fn set_hero_group_equip(
    pool: &SqlitePool,
    user_id: i64,
    group_id: i32,
    index: i32,
    equip_uids: Vec<i64>,
) -> Result<()> {
    let mut transaction = pool.begin().await?;
    let db_group_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM hero_groups_common WHERE user_id = ? AND group_id = ?")
            .bind(user_id)
            .bind(group_id)
            .fetch_optional(&mut *transaction)
            .await?;

    let db_group_id = db_group_id.ok_or_else(|| anyhow::anyhow!("Hero group not found"))?;

    sqlx::query("DELETE FROM hero_group_equips WHERE hero_group_id = ? AND index_slot = ?")
        .bind(db_group_id)
        .bind(index)
        .execute(&mut *transaction)
        .await?;

    for equip_uid in equip_uids {
        sqlx::query(
            "INSERT INTO hero_group_equips (hero_group_id, index_slot, equip_uid) VALUES (?, ?, ?)",
        )
        .bind(db_group_id)
        .bind(index)
        .bind(equip_uid)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(())
}
