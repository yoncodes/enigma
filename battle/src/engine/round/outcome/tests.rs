use super::*;
use crate::test_support::init_config;
use sonettobuf::{FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn configured_limit_counts_played_rounds() {
    init_config();
    let mut fight = Fight {
        battle_id: Some(118353100),
        version: Some(7),
        cur_round: Some(1),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let mut state = RoundState::start(config::configs::get(), &fight);
    assert_eq!(managers.battle_rule.max_round(), Some(20));

    for played in 1..=20 {
        state.begin_round();
        fight.cur_round = Some(state.cur_round);
        let exhausted = played == 20;
        assert_eq!(
            finish_if_battle_ended(&mut state, &fight, &pool, &managers),
            exhausted,
            "after {played} played rounds"
        );
        assert_eq!(state.is_finish, exhausted);
        assert_eq!(
            battle_outcome(&fight, &pool, &managers),
            if exhausted {
                BattleOutcome::OutOfRounds
            } else {
                BattleOutcome::Unfinished
            }
        );
    }
    assert_eq!(fight.cur_round, Some(21));
}

#[test]
fn round_extensions_postpone_exhaustion() {
    init_config();
    let mut fight = Fight {
        battle_id: Some(116330001),
        version: Some(7),
        cur_round: Some(1),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut state = RoundState::start(config::configs::get(), &fight);
    assert_eq!(managers.battle_rule.max_round(), Some(20));

    for played in 1..=24 {
        state.begin_round();
        fight.cur_round = Some(state.cur_round);
        if [5, 10, 15, 20].contains(&played) {
            managers
                .battle_rule
                .add_max_round(crate::engine::manager::battle_rule::MaxRoundCommand {
                    increment: 1,
                    cap: 30,
                })
                .unwrap();
        }
        assert_eq!(
            finish_if_battle_ended(&mut state, &fight, &pool, &managers),
            played == 24,
            "after {played} played rounds"
        );
    }
    assert_eq!(managers.battle_rule.max_round(), Some(24));
    assert_eq!(fight.cur_round, Some(25));
    assert_eq!(
        battle_outcome(&fight, &pool, &managers),
        BattleOutcome::OutOfRounds
    );
}

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

#[test]
fn summoned_replacement_keeps_the_battle_unfinished() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
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
    let summoned = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 30111003,
                position: 2,
            },
        })
        .unwrap()
        .entity
        .uid
        .unwrap();

    assert!(!defenders_defeated(&pool, &managers));
    assert!(!battle_ended(&fight, &pool, &managers));
    assert_eq!(
        battle_outcome(&fight, &pool, &managers),
        BattleOutcome::Unfinished
    );

    managers.hp.lose(summoned, managers.hp.current(summoned), 0);
    assert!(defenders_defeated(&pool, &managers));
    assert!(battle_ended(&fight, &pool, &managers));
}

#[test]
fn restored_depleted_roster_is_terminal() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(0),
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
    let managers = BattleManagers::seeded(&fight);

    assert!(pool.defender_all.is_empty());
    assert!(defenders_defeated(&pool, &managers));
    assert_eq!(
        battle_outcome(&fight, &pool, &managers),
        BattleOutcome::Victory
    );
}
