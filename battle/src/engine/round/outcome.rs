use sonettobuf::Fight;

use crate::engine::{
    manager::{BattleManagers, entity::RosterLane},
    round::state::RoundState,
    skill::target::TargetPool,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleOutcome {
    Victory,
    Defeat,
    OutOfRounds,
    Unfinished,
}

impl BattleOutcome {
    pub const fn winning_team(self) -> Option<i32> {
        match self {
            Self::Victory => Some(1),
            Self::Defeat => Some(2),
            Self::OutOfRounds | Self::Unfinished => None,
        }
    }
}

fn team_defeated(team_type: i32, managers: &BattleManagers) -> bool {
    let roster = managers
        .entity
        .ordered_uids()
        .filter(|uid| managers.entity.team_type(*uid) == Some(team_type))
        .filter(|uid| managers.entity.roster_lane(*uid) == Some(RosterLane::Main))
        .collect::<Vec<_>>();
    !roster.is_empty()
        && roster.iter().all(|uid| managers.hp.current(*uid) <= 0)
        && (team_type != 2
            || roster.iter().all(|uid| {
                !managers
                    .entity
                    .has_pending_reserve_promotion(*uid, &managers.hp)
            }))
}

pub(crate) fn defenders_defeated(_pool: &TargetPool, managers: &BattleManagers) -> bool {
    team_defeated(2, managers)
}

pub(crate) fn attackers_defeated(_pool: &TargetPool, managers: &BattleManagers) -> bool {
    team_defeated(1, managers)
}

pub(crate) fn battle_ended(fight: &Fight, pool: &TargetPool, managers: &BattleManagers) -> bool {
    battle_ended_for_battle_id(fight.battle_id.unwrap_or_default(), pool, managers)
}

pub(crate) fn should_advance_wave(
    fight: &Fight,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> bool {
    defenders_defeated(pool, managers) && !battle_ended(fight, pool, managers)
}

pub(crate) fn battle_ended_for_battle_id(
    battle_id: i32,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> bool {
    terminal_outcome_for_battle_id(battle_id, pool, managers).is_some()
}

pub(crate) fn terminal_outcome_for_battle_id(
    battle_id: i32,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> Option<BattleOutcome> {
    if configured_win_target_defeated(battle_id, managers)
        .unwrap_or_else(|| defenders_defeated(pool, managers) && !managers.wave.has_next_wave())
    {
        return Some(BattleOutcome::Victory);
    }
    attackers_defeated(pool, managers).then_some(BattleOutcome::Defeat)
}

pub(crate) fn battle_outcome(
    fight: &Fight,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> BattleOutcome {
    if configured_win_target_defeated(fight.battle_id.unwrap_or_default(), managers)
        .unwrap_or_else(|| defenders_defeated(pool, managers) && !managers.wave.has_next_wave())
    {
        return BattleOutcome::Victory;
    }
    if attackers_defeated(pool, managers) {
        return BattleOutcome::Defeat;
    }
    if round_limit_reached(fight, managers) {
        BattleOutcome::OutOfRounds
    } else {
        BattleOutcome::Unfinished
    }
}

pub(crate) fn finish_if_battle_ended(
    state: &mut RoundState,
    fight: &Fight,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> bool {
    if battle_ended(fight, pool, managers) || round_limit_reached(fight, managers) {
        state.is_finish = true;
        return true;
    }
    false
}

pub(crate) fn round_limit_reached(fight: &Fight, managers: &BattleManagers) -> bool {
    // Setup starts at 1, and each played round has already advanced the counter.
    managers
        .battle_rule
        .max_round()
        .is_some_and(|max_round| max_round > 0 && fight.cur_round.unwrap_or_default() > max_round)
}

fn configured_win_target_defeated(battle_id: i32, managers: &BattleManagers) -> Option<bool> {
    let model_id = managers.catalog().battle_win_target_model(battle_id)?;
    Some(managers.entity.ordered_uids().any(|uid| {
        managers.entity.model_id(uid) == Some(model_id) && managers.hp.current(uid) <= 0
    }))
}

#[cfg(test)]
mod tests;
