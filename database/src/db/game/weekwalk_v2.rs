use crate::models::game::weekwalk_v2::*;
use anyhow::Result;
use sonettobuf;
use sqlx::SqlitePool;

mod state;

#[cfg(test)]
mod tests;

pub use state::{
    WeekwalkV2BattleSeed, WeekwalkV2LayerSeed, WeekwalkV2Seed, ensure_weekwalk_v2_state,
    mark_layer_finished_shown,
};

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
            params: (!layer.params.is_empty()).then_some(layer.params),
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
            params: (!params.is_empty()).then_some(params),
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
         WHERE user_id = ? AND layer_id = ?
         ORDER BY index_num",
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
        "SELECT layer_id, platinum_cup_num FROM user_weekwalk_v2_prev_settle_layers
         WHERE user_id = ? ORDER BY layer_id",
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
