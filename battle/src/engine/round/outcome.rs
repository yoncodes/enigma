use sonettobuf::Fight;

use crate::engine::{manager::BattleManagers, round::state::RoundState, skill::target::TargetPool};

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

pub(crate) fn defenders_defeated(pool: &TargetPool, managers: &BattleManagers) -> bool {
    !pool.defender_all.is_empty()
        && pool
            .defender_all
            .iter()
            .all(|entity| managers.hp.current(entity.uid) <= 0)
}

pub(crate) fn attackers_defeated(pool: &TargetPool, managers: &BattleManagers) -> bool {
    !pool.attacker_main.is_empty()
        && pool
            .attacker_main
            .iter()
            .all(|entity| managers.hp.current(entity.uid) <= 0)
}

pub(crate) fn battle_ended(fight: &Fight, pool: &TargetPool, managers: &BattleManagers) -> bool {
    battle_ended_for_battle_id(fight.battle_id.unwrap_or_default(), pool, managers)
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
    _pool: &TargetPool,
    managers: &BattleManagers,
) -> BattleOutcome {
    let defenders_defeated = fight.defender.as_ref().is_some_and(|team| {
        let entities = team.entitys.iter().chain(&team.sub_entitys);
        let uids = entities.filter_map(|entity| entity.uid).collect::<Vec<_>>();
        !uids.is_empty() && uids.into_iter().all(|uid| managers.hp.current(uid) <= 0)
    });
    if configured_win_target_defeated(fight.battle_id.unwrap_or_default(), managers)
        .unwrap_or(defenders_defeated && !managers.wave.has_next_wave())
    {
        return BattleOutcome::Victory;
    }
    let attackers_defeated = fight.attacker.as_ref().is_some_and(|team| {
        let uids = team
            .entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>();
        !uids.is_empty() && uids.into_iter().all(|uid| managers.hp.current(uid) <= 0)
    });
    if attackers_defeated {
        return BattleOutcome::Defeat;
    }
    let out_of_rounds = managers
        .catalog()
        .battle_max_round(fight.battle_id.unwrap_or_default())
        .is_some_and(|max_round| max_round > 0 && fight.cur_round.unwrap_or_default() >= max_round);
    if out_of_rounds {
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
    if battle_ended(fight, pool, managers) {
        state.is_finish = true;
        return true;
    }
    false
}

fn configured_win_target_defeated(battle_id: i32, managers: &BattleManagers) -> Option<bool> {
    let model_id = managers.catalog().battle_win_target_model(battle_id)?;
    Some(managers.entity.ordered_uids().any(|uid| {
        managers.entity.model_id(uid) == Some(model_id) && managers.hp.current(uid) <= 0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::init_config;
    use sonettobuf::{FightEntityInfo, FightTeam, HeroAttribute};

    fn depleted_wave(cur_wave: i32) -> (Fight, TargetPool, BattleManagers) {
        init_config();
        let fight = Fight {
            battle_id: Some(2514),
            cur_wave: Some(cur_wave),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1),
                    attr: Some(HeroAttribute {
                        hp: Some(1),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers.hp.lose(-1, 1, 0);
        (fight, pool, managers)
    }

    #[test]
    fn depleted_intermediate_wave_is_not_terminal() {
        let (fight, pool, managers) = depleted_wave(1);

        assert!(defenders_defeated(&pool, &managers));
        assert!(managers.wave.has_next_wave());
        assert!(!battle_ended(&fight, &pool, &managers));
    }

    #[test]
    fn depleted_final_wave_is_terminal() {
        let (fight, pool, managers) = depleted_wave(4);

        assert!(!managers.wave.has_next_wave());
        assert!(battle_ended(&fight, &pool, &managers));
    }

    #[test]
    fn committing_the_outcome_preserves_scheduler_owned_action_points() {
        let (fight, pool, managers) = depleted_wave(4);
        let mut state = RoundState {
            act_point: 3,
            ..Default::default()
        };

        assert!(finish_if_battle_ended(&mut state, &fight, &pool, &managers));
        assert!(state.is_finish);
        assert_eq!(state.act_point, 3);
    }
}
