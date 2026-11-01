use crate::models::game::weekwalk_v2::*;
use anyhow::Result;
use sonettobuf;
use sqlx::SqlitePool;

/// Persist a team's optional skill against the actual current period. The
/// allowed lists were extracted from this client's weekwalk_ver2_time.json.
pub async fn choose_team_skill(pool: &SqlitePool, user_id: i64, no: i32, skills: &[i32]) -> Result<bool> {
    if no <= 0 || skills.len() > 1 { return Ok(false); }
    let periods: std::collections::HashMap<String, Vec<i32>> = serde_json::from_str(
        include_str!("../../../data/weekwalk_v2_period_skills.json"),
    )?;
    let mut tx = pool.begin().await?;
    let time_id: Option<i32> = sqlx::query_scalar("SELECT time_id FROM user_weekwalk_v2_info WHERE user_id = ?")
        .bind(user_id).fetch_optional(&mut *tx).await?;
    let Some(allowed) = time_id.and_then(|id| periods.get(&id.to_string())) else { return Ok(false) };
    if skills.iter().any(|skill| !allowed.contains(skill)) { return Ok(false); }
    sqlx::query("INSERT INTO user_weekwalk_v2_snapshots (user_id, snapshot_no) VALUES (?, ?) ON CONFLICT DO NOTHING")
        .bind(user_id).bind(no).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM user_weekwalk_v2_snapshot_skills WHERE user_id = ? AND snapshot_no = ?")
        .bind(user_id).bind(no).execute(&mut *tx).await?;
    for skill in skills {
        sqlx::query("INSERT INTO user_weekwalk_v2_snapshot_skills (user_id, snapshot_no, skill_id) VALUES (?, ?, ?)")
            .bind(user_id).bind(no).bind(skill).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(true)
}

pub async fn set_battle_team(
    pool: &SqlitePool, user_id: i64, layer_id: i32, battle_id: i32, select: i32,
) -> Result<bool> {
    if select < 0 { return Ok(false); }
    let result = sqlx::query("UPDATE user_weekwalk_v2_battles SET hero_group_select = ? WHERE user_id = ? AND layer_id = ? AND battle_id = ? \
        AND 1 = (SELECT COUNT(*) FROM user_weekwalk_v2_battles WHERE user_id = ? AND layer_id = ? AND battle_id = ?)")
        .bind(select).bind(user_id).bind(layer_id).bind(battle_id)
        .bind(user_id).bind(layer_id).bind(battle_id).execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn clear_layer_finished_notice(pool: &SqlitePool, user_id: i64, layer_id: i32) -> Result<bool> {
    let result = sqlx::query("UPDATE user_weekwalk_v2_layers SET show_finished = FALSE WHERE user_id = ? AND layer_id = ?")
        .bind(user_id).bind(layer_id).execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn clear_rule_notice(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query("UPDATE user_weekwalk_v2_info SET pop_rule = FALSE WHERE user_id = ?")
        .bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn clear_previous_settlement_notice(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query("UPDATE user_weekwalk_v2_prev_settle SET show = FALSE WHERE user_id = ?")
        .bind(user_id).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn weekwalk_team_skill_uses_period_options_and_is_scoped_and_reversible() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        for id in [1, 2, 3] {
            sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
                .bind(id).bind(format!("weekwalk-v2-test-{id}")).execute(&pool).await.unwrap();
        }
        for (id, time_id) in [(1, 1), (2, 2)] {
            sqlx::query("INSERT INTO user_weekwalk_v2_info (user_id, time_id) VALUES (?, ?)")
                .bind(id).bind(time_id).execute(&pool).await.unwrap();
        }
        assert!(choose_team_skill(&pool, 1, 1, &[2004]).await.unwrap());
        assert!(!choose_team_skill(&pool, 2, 1, &[2004]).await.unwrap());
        assert!(!choose_team_skill(&pool, 1, 1, &[2004, 2002]).await.unwrap());
        assert!(!choose_team_skill(&pool, 3, 1, &[]).await.unwrap());
        assert!(!choose_team_skill(&pool, 1, 0, &[2004]).await.unwrap());
        let first = get_weekwalk_v2_info(&pool, 1).await.unwrap();
        assert_eq!(first.snapshot_infos[0].skill_ids, [2004]);
        assert!(get_weekwalk_v2_info(&pool, 2).await.unwrap().snapshot_infos.is_empty());
        assert!(choose_team_skill(&pool, 1, 1, &[]).await.unwrap());
        assert!(get_weekwalk_v2_info(&pool, 1).await.unwrap().snapshot_infos[0].skill_ids.is_empty());
    }
}

pub async fn get_weekwalk_v2_info(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<sonettobuf::WeekwalkVer2Info> {
    // Get main info
    let info = sqlx::query_as::<_, UserWeekwalkV2Info>(
        "SELECT * FROM user_weekwalk_v2_info WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(UserWeekwalkV2Info {
        user_id,
        time_id: 0,
        start_time: 0,
        end_time: 0,
        pop_rule: false,
    });

    // Get layers
    let layer_infos = get_layer_infos(pool, user_id).await?;

    // Get previous settle info
    let prev_settle = get_prev_settle_info(pool, user_id).await?;

    // Get snapshots
    let snapshot_infos = get_snapshot_infos(pool, user_id).await?;

    Ok(sonettobuf::WeekwalkVer2Info {
        time_id: Some(info.time_id),
        start_time: Some(info.start_time),
        end_time: Some(info.end_time),
        layer_infos,
        pop_rule: Some(info.pop_rule),
        prev_settle: Some(prev_settle),
        snapshot_infos: snapshot_infos.into_iter().map(Into::into).collect(),
    })
}

async fn get_layer_infos(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<sonettobuf::WeekwalkVer2LayerInfo>> {
    let layers = sqlx::query_as::<_, WeekwalkV2Layer>(
        "SELECT layer_id, scene_id, all_pass, finished, unlock, show_finished, params
         FROM user_weekwalk_v2_layers WHERE user_id = ? ORDER BY layer_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut layer_infos = Vec::new();
    for layer in layers {
        // Get battles
        let battle_infos = get_layer_battles(pool, user_id, layer.layer_id).await?;

        // Get elements
        let element_infos = get_layer_elements(pool, user_id, layer.layer_id).await?;

        layer_infos.push(sonettobuf::WeekwalkVer2LayerInfo {
            id: Some(layer.layer_id),
            scene_id: Some(layer.scene_id),
            all_pass: Some(layer.all_pass),
            finished: Some(layer.finished),
            unlock: Some(layer.unlock),
            show_finished: Some(layer.show_finished),
            battle_infos,
            element_infos: element_infos.into_iter().map(Into::into).collect(),
            params: Some(layer.params),
        });
    }

    Ok(layer_infos)
}

async fn get_layer_battles(
    pool: &SqlitePool,
    user_id: i64,
    layer_id: i32,
) -> Result<Vec<sonettobuf::WeekwalkVer2BattleInfo>> {
    let battles: Vec<(i32, i32, i32, i32, String)> = sqlx::query_as(
        "SELECT battle_id, status, hero_group_select, element_id, params
         FROM user_weekwalk_v2_battles
         WHERE user_id = ? AND layer_id = ?
         ORDER BY battle_id",
    )
    .bind(user_id)
    .bind(layer_id)
    .fetch_all(pool)
    .await?;

    let mut battle_infos = Vec::new();
    for (battle_id, status, hero_group_select, element_id, params) in battles {
        // Get hero IDs
        let hero_ids = sqlx::query_scalar(
            "SELECT hero_id FROM user_weekwalk_v2_battle_heroes
             WHERE user_id = ? AND layer_id = ? AND battle_id = ?",
        )
        .bind(user_id)
        .bind(layer_id)
        .bind(battle_id)
        .fetch_all(pool)
        .await?;

        // Get choose skill IDs
        let choose_skill_ids = sqlx::query_scalar(
            "SELECT skill_id FROM user_weekwalk_v2_battle_skills
             WHERE user_id = ? AND layer_id = ? AND battle_id = ?",
        )
        .bind(user_id)
        .bind(layer_id)
        .bind(battle_id)
        .fetch_all(pool)
        .await?;

        // Get cup infos
        let cups: Vec<(i32, i32)> = sqlx::query_as(
            "SELECT cup_id, result FROM user_weekwalk_v2_cups
             WHERE user_id = ? AND layer_id = ? AND battle_id = ?",
        )
        .bind(user_id)
        .bind(layer_id)
        .bind(battle_id)
        .fetch_all(pool)
        .await?;

        let cup_infos = cups
            .into_iter()
            .map(|(cup_id, result)| WeekwalkV2CupInfo { cup_id, result })
            .collect::<Vec<_>>();

        battle_infos.push(sonettobuf::WeekwalkVer2BattleInfo {
            battle_id: Some(battle_id),
            status: Some(status),
            hero_ids,
            choose_skill_ids,
            hero_group_select: Some(hero_group_select),
            element_id: Some(element_id),
            cup_infos: cup_infos.into_iter().map(Into::into).collect(),
            params: Some(params),
        });
    }

    Ok(battle_infos)
}

async fn get_layer_elements(
    pool: &SqlitePool,
    user_id: i64,
    layer_id: i32,
) -> Result<Vec<WeekwalkV2ElementInfo>> {
    let elements: Vec<(i32, bool, i32, bool)> = sqlx::query_as(
        "SELECT element_id, finish, index_num, visible
         FROM user_weekwalk_v2_elements
         WHERE user_id = ? AND layer_id = ?",
    )
    .bind(user_id)
    .bind(layer_id)
    .fetch_all(pool)
    .await?;

    Ok(elements
        .into_iter()
        .map(
            |(element_id, finish, index_num, visible)| WeekwalkV2ElementInfo {
                element_id,
                finish,
                index_num,
                visible,
            },
        )
        .collect())
}

async fn get_prev_settle_info(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<sonettobuf::WeekwalkVer2PrevSettleInfo> {
    let settle = sqlx::query_as::<_, WeekwalkV2PrevSettle>(
        "SELECT * FROM user_weekwalk_v2_prev_settle WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(WeekwalkV2PrevSettle {
        user_id,
        max_layer_id: 0,
        max_battle_id: 0,
        max_battle_index: 0,
        show: false,
    });

    // Get layer infos
    let layer_infos: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT layer_id, platinum_cup_num FROM user_weekwalk_v2_prev_settle_layers WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(sonettobuf::WeekwalkVer2PrevSettleInfo {
        max_layer_id: Some(settle.max_layer_id),
        max_battle_id: Some(settle.max_battle_id),
        max_battle_index: Some(settle.max_battle_index),
        show: Some(settle.show),
        layer_infos: layer_infos
            .into_iter()
            .map(
                |(layer_id, platinum_cup_num)| WeekwalkV2PrevSettleLayerInfo {
                    layer_id,
                    platinum_cup_num,
                },
            )
            .map(Into::into)
            .collect(),
    })
}

async fn get_snapshot_infos(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<WeekwalkV2SnapshotInfo>> {
    let snapshots: Vec<i32> = sqlx::query_scalar(
        "SELECT snapshot_no FROM user_weekwalk_v2_snapshots WHERE user_id = ? ORDER BY snapshot_no",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut snapshot_infos = Vec::new();
    for snapshot_no in snapshots {
        let skill_ids = sqlx::query_scalar(
            "SELECT skill_id FROM user_weekwalk_v2_snapshot_skills WHERE user_id = ? AND snapshot_no = ?"
        )
        .bind(user_id)
        .bind(snapshot_no)
        .fetch_all(pool)
        .await?;

        snapshot_infos.push(WeekwalkV2SnapshotInfo {
            snapshot_no,
            skill_ids,
        });
    }

    Ok(snapshot_infos)
}
