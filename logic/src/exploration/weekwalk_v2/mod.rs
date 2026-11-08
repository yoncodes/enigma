use crate::error::AppError;
use common::time::ServerTime;
use database::db::game::{
    hero_groups,
    weekwalk_v2::{self, WeekwalkV2BattleSeed, WeekwalkV2LayerSeed, WeekwalkV2Seed},
};
use sonettobuf::{
    WeekwalkVer2ChangeHeroGroupSelectReply, WeekwalkVer2ChooseSkillReply, WeekwalkVer2GetInfoReply,
    WeekwalkVer2MarkShowFinishedReply,
};
use sqlx::SqlitePool;

#[cfg(test)]
mod tests;

pub(super) async fn info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<WeekwalkVer2GetInfoReply, AppError> {
    info_at(db, player_id, ServerTime::now_ms()).await
}

pub(super) async fn mark_finished_shown(
    db: &SqlitePool,
    player_id: i64,
    layer_id: i32,
) -> Result<WeekwalkVer2MarkShowFinishedReply, AppError> {
    mark_finished_shown_at(db, player_id, layer_id, ServerTime::now_ms()).await
}

pub(super) async fn change_group(
    db: &SqlitePool,
    player_id: i64,
    layer_id: i32,
    battle_id: i32,
    select: i32,
) -> Result<WeekwalkVer2ChangeHeroGroupSelectReply, AppError> {
    change_group_at(
        db,
        player_id,
        layer_id,
        battle_id,
        select,
        ServerTime::now_ms(),
    )
    .await
}

pub(super) async fn choose_skill(
    db: &SqlitePool,
    player_id: i64,
    no: i32,
    skill_ids: Vec<i32>,
) -> Result<WeekwalkVer2ChooseSkillReply, AppError> {
    choose_skill_at(db, player_id, no, skill_ids, ServerTime::now_ms()).await
}

async fn info_at(
    db: &SqlitePool,
    player_id: i64,
    now_ms: i64,
) -> Result<WeekwalkVer2GetInfoReply, AppError> {
    ensure_at(db, player_id, now_ms).await?;
    Ok(WeekwalkVer2GetInfoReply {
        info: Some(weekwalk_v2::get_weekwalk_v2_info(db, player_id).await?),
    })
}

async fn mark_finished_shown_at(
    db: &SqlitePool,
    player_id: i64,
    layer_id: i32,
    now_ms: i64,
) -> Result<WeekwalkVer2MarkShowFinishedReply, AppError> {
    ensure_at(db, player_id, now_ms).await?;
    if !weekwalk_v2::mark_layer_finished_shown(db, player_id, layer_id).await? {
        return Err(AppError::InvalidRequest);
    }
    Ok(WeekwalkVer2MarkShowFinishedReply {
        layer_id: Some(layer_id),
    })
}

async fn change_group_at(
    db: &SqlitePool,
    player_id: i64,
    layer_id: i32,
    battle_id: i32,
    select: i32,
    now_ms: i64,
) -> Result<WeekwalkVer2ChangeHeroGroupSelectReply, AppError> {
    ensure_at(db, player_id, now_ms).await?;
    if select <= 0
        || hero_groups::get_hero_group(db, player_id, select)
            .await?
            .is_none()
    {
        return Err(AppError::InvalidRequest);
    }
    if !weekwalk_v2::set_battle_group(db, player_id, layer_id, battle_id, select).await? {
        return Err(AppError::InvalidRequest);
    }
    Ok(WeekwalkVer2ChangeHeroGroupSelectReply {
        layer_id: Some(layer_id),
        battle_id: Some(battle_id),
        select: Some(select),
    })
}

async fn choose_skill_at(
    db: &SqlitePool,
    player_id: i64,
    no: i32,
    skill_ids: Vec<i32>,
    now_ms: i64,
) -> Result<WeekwalkVer2ChooseSkillReply, AppError> {
    ensure_at(db, player_id, now_ms).await?;
    if no <= 0
        || hero_groups::get_hero_group(db, player_id, no)
            .await?
            .is_none()
    {
        return Err(AppError::InvalidRequest);
    }
    let skill_id = match skill_ids.as_slice() {
        [] => None,
        [skill_id] => Some(*skill_id),
        _ => return Err(AppError::InvalidRequest),
    };
    if let Some(skill_id) = skill_id {
        let seed = seed_at(now_ms)?;
        let game = config::get();
        let unlocked = weekwalk_v2::unlocked_layer_ids(db, player_id).await?;
        let available = seed
            .layers
            .iter()
            .filter(|layer| unlocked.contains(&layer.layer_id))
            .filter_map(|layer| game.weekwalk_ver2.get(layer.layer_id))
            .max_by_key(|layer| layer.layer)
            .ok_or(AppError::InvalidRequest)?
            .choose_skill_num;
        let time = game
            .weekwalk_ver2_time
            .iter()
            .find(|time| time.time_id == seed.time_id)
            .ok_or(AppError::InvalidRequest)?;
        let allowed = time
            .optional_skills
            .split('#')
            .take(usize::try_from(available).map_err(|_| AppError::InvalidRequest)?)
            .map(|id| id.parse::<i32>().map_err(|_| AppError::InvalidRequest))
            .collect::<Result<Vec<_>, _>>()?;
        if !allowed.contains(&skill_id) || game.weekwalk_ver2_skill.get(skill_id).is_none() {
            return Err(AppError::InvalidRequest);
        }
    }
    weekwalk_v2::replace_snapshot_skills(db, player_id, no, &skill_ids).await?;
    Ok(WeekwalkVer2ChooseSkillReply {
        no: Some(no),
        skill_ids,
    })
}

async fn ensure_at(db: &SqlitePool, player_id: i64, now_ms: i64) -> Result<(), AppError> {
    let seed = seed_at(now_ms)?;
    weekwalk_v2::ensure_weekwalk_v2_state(db, player_id, &seed).await?;
    Ok(())
}

fn seed_at(now_ms: i64) -> Result<WeekwalkV2Seed, AppError> {
    let schedule = common::config()
        .weekwalk_v2
        .schedule
        .iter()
        .find(|schedule| schedule.start_time <= now_ms && now_ms <= schedule.end_time)
        .ok_or_else(|| AppError::Custom("no active Weekwalk v2 schedule".into()))?;
    let game = config::get();
    let time = game
        .weekwalk_ver2_time
        .iter()
        .find(|time| time.time_id == schedule.time_id)
        .ok_or_else(|| AppError::Custom("Weekwalk v2 schedule has no time config".into()))?;
    let mut layer_configs = game
        .weekwalk_ver2
        .iter()
        .filter(|layer| layer.issue_id == time.issue_id)
        .collect::<Vec<_>>();
    layer_configs.sort_by_key(|layer| layer.layer);
    if layer_configs.len() != schedule.layers.len() {
        return Err(AppError::Custom(
            "Weekwalk v2 schedule layer count does not match game config".into(),
        ));
    }

    let mut layers = Vec::with_capacity(layer_configs.len());
    for layer in layer_configs {
        let mapped = schedule
            .layers
            .iter()
            .find(|mapped| mapped.layer_id == layer.id)
            .ok_or_else(|| AppError::Custom("Weekwalk v2 layer has no element mapping".into()))?;
        let front = game
            .weekwalk_ver2_element
            .get(mapped.front_element_id)
            .filter(|element| element.round_id == 1)
            .ok_or_else(|| AppError::Custom("invalid Weekwalk v2 front element".into()))?;
        let rear = game
            .weekwalk_ver2_element
            .get(mapped.rear_element_id)
            .filter(|element| element.round_id == 2)
            .ok_or_else(|| AppError::Custom("invalid Weekwalk v2 rear element".into()))?;
        layers.push(WeekwalkV2LayerSeed {
            layer_id: layer.id,
            scene_id: layer.scene_id,
            unlock: layer.pre_id == 0,
            battles: vec![
                WeekwalkV2BattleSeed {
                    battle_id: layer.fight_id_front,
                    element_id: front.id,
                    index: front.round_id,
                    visible: true,
                },
                WeekwalkV2BattleSeed {
                    battle_id: layer.fight_id_rear,
                    element_id: rear.id,
                    index: rear.round_id,
                    visible: false,
                },
            ],
        });
    }

    let previous_layer_ids = game
        .weekwalk_ver2_time
        .iter()
        .filter(|candidate| candidate.time_id < time.time_id)
        .max_by_key(|candidate| candidate.time_id)
        .map(|previous| {
            let mut ids = game
                .weekwalk_ver2
                .iter()
                .filter(|layer| layer.issue_id == previous.issue_id)
                .map(|layer| (layer.layer, layer.id))
                .collect::<Vec<_>>();
            ids.sort_unstable();
            ids.into_iter().map(|(_, id)| id).collect()
        })
        .unwrap_or_default();

    Ok(WeekwalkV2Seed {
        time_id: schedule.time_id,
        start_time: schedule.start_time,
        end_time: schedule.end_time,
        pop_rule: schedule.pop_rule,
        layers,
        previous_layer_ids,
    })
}
