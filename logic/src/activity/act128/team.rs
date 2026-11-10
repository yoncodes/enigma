use super::{ensure_act128_activity, preserve_max_total_latest_in_transaction};
use crate::{error::AppError, profile::ProfileManager};
use database::{
    db::game::{
        activity_state::{self, ActivityStateKind, ActivityStateSet},
        friends,
    },
    models::game::heros::get_hero_by_uid,
};
use serde::{Deserialize, Serialize};
use sonettobuf::{
    Act128ActModeHeroInfo, Act128ActModeHeroSlot, Act128ActModeTeam, ResetAct128TeamReply,
    SetAct128TeamReply,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

const SLOTS_PER_LINE: usize = 4;

#[derive(Serialize, Deserialize)]
struct StoredTeam {
    front: Vec<Act128ActModeHeroSlot>,
    back: Vec<Act128ActModeHeroSlot>,
    add_bond_group_id: i32,
}

pub(super) async fn set(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    boss_id: i32,
    front: Vec<Act128ActModeHeroSlot>,
    back: Vec<Act128ActModeHeroSlot>,
    add_bond_group_id: i32,
) -> Result<SetAct128TeamReply, AppError> {
    validate_route(activity_id, boss_id)?;
    let stored = StoredTeam {
        front: front.into_iter().map(normalize).collect(),
        back: back.into_iter().map(normalize).collect(),
        add_bond_group_id,
    };
    let projected = validate_and_project(db, player_id, activity_id, &stored).await?;
    let ext = serde_json::to_string(&stored)?;
    save(
        db,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act128ModeTeam,
            entry_id: boss_id,
            state: 0,
            progress: 0,
            ext: &ext,
        },
    )
    .await?;

    Ok(SetAct128TeamReply {
        activity_id: Some(activity_id),
        boss_id: Some(boss_id),
        act_mode_team: Some(projected.team),
    })
}

pub(super) async fn reset(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    boss_id: i32,
) -> Result<ResetAct128TeamReply, AppError> {
    validate_route(activity_id, boss_id)?;
    save(
        db,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act128ModeTeam,
            entry_id: boss_id,
            state: 0,
            progress: 0,
            ext: "",
        },
    )
    .await?;

    Ok(ResetAct128TeamReply {
        activity_id: Some(activity_id),
        boss_id: Some(boss_id),
        act_mode_team: Some(Default::default()),
        latest_point: Some(0),
    })
}

async fn save(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    state: ActivityStateSet<'_>,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    preserve_max_total_latest_in_transaction(&mut tx, player_id, activity_id).await?;
    activity_state::set_in_transaction(&mut tx, player_id, activity_id, state).await?;
    tx.commit().await?;
    Ok(())
}

pub(super) async fn from_state(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    boss_id: i32,
    ext: &str,
) -> Result<Act128ActModeTeam, AppError> {
    if ext.is_empty() {
        return Ok(Default::default());
    }
    validate_route(activity_id, boss_id)?;
    Ok(
        validate_and_project(db, player_id, activity_id, &serde_json::from_str(ext)?)
            .await?
            .team,
    )
}

struct Projection {
    team: Act128ActModeTeam,
    bond_heroes: Vec<config::Activity128BondHero>,
}

async fn validate_and_project(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stored: &StoredTeam,
) -> Result<Projection, AppError> {
    if stored.front.len() != SLOTS_PER_LINE || stored.back.len() != SLOTS_PER_LINE {
        return Err(AppError::InvalidRequest);
    }
    let projected = project(db, player_id, stored).await?;
    validate_add_bond(
        activity_id,
        stored.add_bond_group_id,
        &projected.bond_heroes,
    )?;
    Ok(projected)
}

async fn project(
    db: &SqlitePool,
    player_id: i64,
    stored: &StoredTeam,
) -> Result<Projection, AppError> {
    let friend_ids = friends::get_friend_ids(db, player_id)
        .await?
        .into_iter()
        .map(|id| id as i64)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut hero_ids = HashSet::new();
    let mut bond_heroes = Vec::new();
    let mut assist_count = 0;
    let front_hero_infos = project_slots(
        db,
        player_id,
        &stored.front,
        &friend_ids,
        &mut seen,
        &mut hero_ids,
        &mut bond_heroes,
        &mut assist_count,
    )
    .await?;
    let back_hero_infos = project_slots(
        db,
        player_id,
        &stored.back,
        &friend_ids,
        &mut seen,
        &mut hero_ids,
        &mut bond_heroes,
        &mut assist_count,
    )
    .await?;
    Ok(Projection {
        team: Act128ActModeTeam {
            front_hero_infos,
            back_hero_infos,
            add_bond_group_id: Some(stored.add_bond_group_id),
        },
        bond_heroes,
    })
}

async fn project_slots(
    db: &SqlitePool,
    player_id: i64,
    slots: &[Act128ActModeHeroSlot],
    friend_ids: &HashSet<i64>,
    seen: &mut HashSet<i64>,
    hero_ids: &mut HashSet<i32>,
    bond_heroes: &mut Vec<config::Activity128BondHero>,
    assist_count: &mut usize,
) -> Result<Vec<Act128ActModeHeroInfo>, AppError> {
    let mut infos = Vec::with_capacity(slots.len());
    for slot in slots {
        let uid = slot.uid.unwrap_or_default();
        let external = slot.is_friend.unwrap_or(false);
        let owner_id = slot.friend_user_id.unwrap_or_default();
        if uid == 0 {
            if external || owner_id != 0 {
                return Err(AppError::InvalidRequest);
            }
            infos.push(Act128ActModeHeroInfo {
                uid: Some(0),
                is_friend: Some(false),
                assist_hero_info: None,
                tag1s: Vec::new(),
                tag2s: Vec::new(),
            });
            continue;
        }
        if uid < 0 || !seen.insert(uid) {
            return Err(AppError::InvalidRequest);
        }
        let hero = get_hero_by_uid(db, uid)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        if !hero_ids.insert(hero.record.hero_id) {
            return Err(AppError::InvalidRequest);
        }
        bond_heroes.push(
            config::configs::get()
                .activity128_bond_hero(hero.record.hero_id, hero.record.destiny_stone)
                .ok_or(AppError::InvalidRequest)?,
        );
        let assist_hero_info = if external {
            *assist_count += 1;
            if *assist_count > 1
                || owner_id <= 0
                || owner_id == player_id
                || hero.record.user_id != owner_id
            {
                return Err(AppError::InvalidRequest);
            }
            let info = ProfileManager::new(owner_id)
                .assist_hero(db, hero.record.hero_id, friend_ids.contains(&owner_id))
                .await?
                .map(|(_, info)| info)
                .filter(|info| info.hero_uid == Some(uid))
                .ok_or(AppError::InvalidRequest)?;
            Some(info)
        } else {
            if owner_id != 0 || hero.record.user_id != player_id {
                return Err(AppError::InvalidRequest);
            }
            None
        };
        infos.push(Act128ActModeHeroInfo {
            uid: Some(uid),
            is_friend: Some(external),
            assist_hero_info,
            tag1s: Vec::new(),
            tag2s: Vec::new(),
        });
    }
    Ok(infos)
}

fn validate_route(activity_id: i32, boss_id: i32) -> Result<(), AppError> {
    ensure_act128_activity(activity_id)?;
    let tables = config::configs::get();
    if !tables
        .activity128_episode
        .iter()
        .any(|row| row.activity_id == activity_id && row.stage == boss_id)
    {
        return Err(AppError::InvalidRequest);
    }
    Ok(())
}

fn validate_add_bond(
    activity_id: i32,
    selected_group_id: i32,
    heroes: &[config::Activity128BondHero],
) -> Result<(), AppError> {
    let tables = config::configs::get();
    let mut special_active = false;
    let mut candidates = Vec::new();
    for group in tables
        .activity128_bond_groups(activity_id)
        .ok_or(AppError::InvalidRequest)?
    {
        let count = heroes.iter().filter(|hero| group.matches(hero)).count();
        if group.enables_added_bond() {
            special_active = count > 0;
        }
        if group.is_battle_tag_group() && count > 0 && count < group.max_active_num {
            candidates.push((count, group.group_id));
        }
    }
    let max = candidates.iter().map(|(count, _)| *count).max();
    let valid = if !special_active || max.is_none() {
        selected_group_id == 0
    } else {
        max.is_some_and(|max| {
            candidates
                .iter()
                .any(|(count, group_id)| *count == max && *group_id == selected_group_id)
        })
    };
    valid.then_some(()).ok_or(AppError::InvalidRequest)
}

fn normalize(slot: Act128ActModeHeroSlot) -> Act128ActModeHeroSlot {
    let friend_user_id = slot.friend_user_id.unwrap_or_default();
    Act128ActModeHeroSlot {
        uid: Some(slot.uid.unwrap_or_default()),
        is_friend: Some(slot.is_friend.unwrap_or(false) || friend_user_id != 0),
        friend_user_id: Some(friend_user_id),
    }
}

#[cfg(test)]
mod tests;
