use crate::engine::{
    manager::{BattleManagers, buff::BuffDurationAdvance},
    skill::buff_act::effect_time::ROUND_START_CARD_REACTION_DURATION,
};

pub(crate) fn opening_duration(
    managers: &BattleManagers,
    owner_uids: &[i64],
) -> Option<BuffDurationAdvance> {
    if owner_uids.is_empty() {
        return None;
    }
    duration_snapshot(managers, owner_uids, ROUND_START_CARD_REACTION_DURATION)
}

pub(super) fn duration_snapshot(
    managers: &BattleManagers,
    owner_uids: &[i64],
    take_stage: i32,
) -> Option<BuffDurationAdvance> {
    let buff_uids = managers.buff.duration_buff_uids(take_stage, owner_uids);
    if buff_uids.is_empty() {
        return None;
    }
    BuffDurationAdvance::new(take_stage, owner_uids.to_vec(), Some(buff_uids))
}
