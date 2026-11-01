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
    // The target pool is a snapshot from the beginning of the current phase.
    // Defeating an enemy can summon ordinary replacements before that phase
    // ends. Newly added special entities keep their existing wave semantics.
    !pool.defender_all.is_empty()
        && pool
            .defender_all
            .iter()
            .all(|entity| managers.hp.current(entity.uid) <= 0)
        && !managers.entity.ordered_uids().any(|uid| {
            managers.entity.team_type(uid) == Some(2)
                && matches!(
                    managers.entity.roster_lane(uid),
                    Some(
                        crate::engine::manager::entity::RosterLane::Main
                            | crate::engine::manager::entity::RosterLane::Reserve
                    )
                )
                && managers.hp.current(uid) > 0
        })
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
    managers
        .battle_rule
        .max_round()
        // The fight starts at round 1; resolving the first action advances it to 2.
        // `max_round` counts playable rounds, so the last allowed action has only
        // completed once this internal counter advances beyond the configured limit.
        .is_some_and(|max_round| max_round > 0 && fight.cur_round.unwrap_or_default() > max_round)
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
    fn tts13_summoned_stones_must_be_defeated_before_victory() {
        use crate::engine::manager::entity::{EntityCommand, EntityOperation};
        use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};

        init_config();
        let boss = crate::engine::fight::defender::Defender::build_monster_with_uid(
            119_381_070, -1, 1, 2,
        )
        .unwrap();
        let mut fight = Fight {
            battle_id: Some(119_381_070),
            cur_wave: Some(1),
            cur_round: Some(1),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam { entitys: vec![boss], ..Default::default() }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut stones = Vec::new();
        for position in [2, 3] {
            let entity = managers.execute_entity(EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60008, "Summon"),
                },
                source_uid: -1,
                target_uid: -1,
                operation: EntityOperation::SummonCombatant { model_id: 119_381_060, position },
            }).unwrap().entity;
            stones.push(entity.uid.unwrap());
        }
        managers.hp.lose(-1, i32::MAX, 10);
        assert!(stones.iter().all(|uid| managers.hp.current(*uid) > 0));
        assert!(!defenders_defeated(&pool, &managers), "live spawned enemies must prevent victory");
        assert_eq!(terminal_outcome_for_battle_id(119_381_070, &pool, &managers), None);
        let mut defeated_player = managers.clone();
        defeated_player.hp.lose(10, i32::MAX, -1);
        assert_eq!(
            terminal_outcome_for_battle_id(119_381_070, &pool, &defeated_player),
            Some(BattleOutcome::Defeat),
            "a real defeat must remain a defeat while the spawned enemies live",
        );
        managers.sync_entities(&mut fight);
        assert_eq!(battle_outcome(&fight, &pool, &managers), BattleOutcome::Unfinished);

        for uid in stones { managers.hp.lose(uid, i32::MAX, 10); }
        assert!(defenders_defeated(&pool, &managers));
        assert_eq!(terminal_outcome_for_battle_id(119_381_070, &pool, &managers), Some(BattleOutcome::Victory));
        managers.sync_entities(&mut fight);
        assert_eq!(battle_outcome(&fight, &pool, &managers), BattleOutcome::Victory);
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

    #[test]
    fn instruction_round_limits_allow_every_configured_action_round() {
        init_config();
        let db = crate::test_support::game_data();
        let levels = db.instruction_level.iter().collect::<Vec<_>>();
        assert_eq!(levels.len(), 19);
        for level in levels {
            let episode = db.episode.get(level.episode_id).expect("instruction episode");
            let config = db.battle.get(episode.battle_id).expect("instruction battle");
            assert!(config.max_round > 0);
            let mut fight = Fight { battle_id: Some(config.id), cur_round: Some(1), ..Default::default() };
            let managers = BattleManagers::seeded(&fight);
            assert_eq!(managers.battle_rule.max_round(), Some(config.max_round));
            for completed in 0..config.max_round {
                fight.cur_round = Some(completed + 1);
                assert!(!round_limit_reached(&fight, &managers), "instruction {} ended after {} of {} rounds", level.id, completed, config.max_round);
            }
            fight.cur_round = Some(config.max_round + 1);
            assert!(round_limit_reached(&fight, &managers), "instruction {} did not end after its configured limit", level.id);
        }
        let coop = db.instruction_level.get(601).unwrap();
        assert_eq!(db.episode.get(coop.episode_id).unwrap().battle_id, 5126);
        assert_eq!(db.battle.get(5126).unwrap().max_round, 2);
    }
}
