use crate::types::{activity_id::ActivityId, copost_const_id::CopostConstId};
use chrono::{NaiveDateTime, TimeZone, Utc};
use sonettobuf::ActivityInfo;

const PERMANENT_END_MS: u64 = 2_145_934_800_000;
const PERMANENT_RETRO_TYPE: i32 = 2;

pub(super) fn catalog_infos() -> Vec<ActivityInfo> {
    config::configs::get()
        .activity
        .iter()
        .map(|activity| activity_info(activity.id))
        .collect()
}

fn is_open(open_id: i32) -> bool {
    open_id == 0
        || config::configs::get()
            .open
            .get(open_id)
            .is_some_and(|open| open.is_online != 0)
}

pub(super) fn apply_bp_activity(infos: &mut Vec<ActivityInfo>) {
    let Some(bp) = database::db::game::tasks::current_battle_pass() else {
        return;
    };
    if bp.activity_id <= 0 {
        return;
    }

    if let Some(info) = infos
        .iter_mut()
        .find(|info| info.id == Some(bp.activity_id as u32))
    {
        *info = activity_info(bp.activity_id);
    } else {
        infos.push(activity_info(bp.activity_id));
    }
}

pub(super) fn apply_act125_activity(infos: &mut Vec<ActivityInfo>) {
    for activity_id in act125_activity_ids() {
        if infos.iter().all(|info| info.id != Some(activity_id as u32)) {
            infos.push(activity_info(activity_id));
        }
    }
}

pub(super) fn default_act125_activity_id() -> Option<i32> {
    act125_activity_ids().into_iter().next()
}

fn act125_activity_ids() -> Vec<i32> {
    let tables = config::configs::get();
    let mut activity_ids = tables
        .activity125
        .iter()
        .map(|row| row.activity_id)
        .filter(|activity_id| is_activity_online(*activity_id))
        .collect::<Vec<_>>();

    activity_ids.sort_unstable_by(|left, right| right.cmp(left));
    activity_ids.dedup();
    activity_ids
}

pub(super) fn latest_act101_activity_id() -> i32 {
    latest_config_activity_id(
        config::configs::get()
            .activity101
            .iter()
            .map(|row| row.activity_id),
        ActivityId::SilverLitNight,
    )
}

pub(super) fn active_act101_activity_ids_at(now_ms: u64) -> Vec<i32> {
    let mut activity_ids = config::configs::get()
        .activity101
        .iter()
        .map(|row| row.activity_id)
        .filter(|activity_id| {
            super::schedule::get(*activity_id).is_some_and(|schedule| {
                schedule.start_time <= now_ms
                    && now_ms <= schedule.end_time
                    && config::configs::get()
                        .activity
                        .get(*activity_id)
                        .is_none_or(|activity| is_open(activity.open_id))
            })
        })
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();
    activity_ids
}

pub(super) fn latest_act160_activity_id() -> i32 {
    latest_config_activity_id(
        config::configs::get()
            .activity160_mission
            .iter()
            .map(|row| row.activity_id),
        ActivityId::GiftOfTheBeginning,
    )
}

pub(super) fn latest_act165_activity_id() -> i32 {
    latest_config_activity_id(
        config::configs::get()
            .activity165_story
            .iter()
            .map(|row| row.activity_id),
        ActivityId::StoryDeduction,
    )
}

pub(super) fn latest_act212_activity_id() -> i32 {
    latest_config_activity_id(
        config::configs::get()
            .activity212_bonus
            .iter()
            .map(|row| row.activity_id),
        ActivityId::ManyFacesOfParis,
    )
}

pub(super) fn latest_act228_activity_id() -> i32 {
    latest_config_activity_id(
        config::configs::get()
            .activity228
            .iter()
            .map(|row| row.activity_id),
        ActivityId::MoonlightGardening,
    )
}

fn latest_config_activity_id(ids: impl Iterator<Item = i32>, fallback: ActivityId) -> i32 {
    let mut candidates = ids.filter(|id| *id > 0).collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();

    candidates
        .iter()
        .rev()
        .copied()
        .find(|activity_id| {
            config::configs::get()
                .activity
                .get(*activity_id)
                .is_some_and(|activity| is_open(activity.open_id))
        })
        .or_else(|| candidates.last().copied())
        .unwrap_or_else(|| fallback.id())
}

fn activity_info(activity_id: i32) -> ActivityInfo {
    let (start_time, end_time) = activity_time_range(activity_id);
    let online = is_activity_online(activity_id);

    ActivityInfo {
        id: Some(activity_id as u32),
        start_time: Some(start_time),
        end_time: Some(end_time),
        online: Some(online),
        is_new_stage: Some(false),
        current_stage: Some(0),
        is_unlock: Some(is_unlocked_by_default(activity_id)),
        is_receive_all_bonus: Some(false),
    }
}

fn is_activity_online(activity_id: i32) -> bool {
    let is_scheduled = super::schedule::get(activity_id).is_some();

    is_scheduled
        && config::configs::get()
            .activity
            .get(activity_id)
            .is_none_or(|activity| is_open(activity.open_id))
}

fn activity_time_range(activity_id: i32) -> (u64, u64) {
    if let Some(schedule) = super::schedule::get(activity_id) {
        return (schedule.start_time, schedule.end_time);
    }

    let is_permanent = config::configs::get()
        .activity
        .get(activity_id)
        .is_some_and(|activity| activity.is_retro_acitivity == PERMANENT_RETRO_TYPE);
    if is_permanent {
        return (0, PERMANENT_END_MS);
    }

    version_activity_time_range().unwrap_or((0, PERMANENT_END_MS))
}

fn version_activity_time_range() -> Option<(u64, u64)> {
    let tables = config::configs::get();
    let start = tables
        .copost_const
        .get(CopostConstId::ActivityStartTime.id())
        .and_then(|row| parse_config_time_millis(&row.value2));
    let end = tables
        .copost_const
        .get(CopostConstId::ActivityEndTime.id())
        .and_then(|row| parse_config_time_millis(&row.value2));

    start.zip(end).filter(|(start, end)| start < end)
}

fn parse_config_time_millis(value: &str) -> Option<u64> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");

    ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %-H:%M:%S"]
        .into_iter()
        .find_map(|format| NaiveDateTime::parse_from_str(&normalized, format).ok())
        .and_then(|time| {
            Utc.from_utc_datetime(&time)
                .timestamp_millis()
                .try_into()
                .ok()
        })
}

pub(super) fn is_unlocked_by_default(activity_id: i32) -> bool {
    if let Some(schedule) = super::schedule::get(activity_id) {
        return schedule.is_unlock;
    }

    match config::configs::get().activity.get(activity_id) {
        Some(activity) => activity.is_retro_acitivity != PERMANENT_RETRO_TYPE,
        None => true,
    }
}

#[cfg(test)]
mod tests;
