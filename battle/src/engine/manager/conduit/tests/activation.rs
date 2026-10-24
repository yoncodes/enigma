use super::*;

#[test]
fn unique_activation_without_energy_mutation_uses_the_current_team_cost() {
    crate::test_support::init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    ex_skill_level: Some(1),
                    ..Default::default()
                },
                entity(11, 3145),
                FightEntityInfo {
                    uid: Some(12),
                    model_id: Some(3025),
                    destiny_stone: Some(302502),
                    destiny_rank: Some(4),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    let complete = |manager: &mut ConduitManager, source_uid, skill_id| {
        manager
            .execute(ConduitCommand::BeginSkill {
                source_uid,
                skill_id,
                cost_reduction: 0,
            })
            .unwrap();
        manager
            .execute(ConduitCommand::CommitSkillCost {
                source_uid,
                skill_id,
            })
            .unwrap();
        manager
            .execute(ConduitCommand::CompleteActivation {
                source_uid,
                skill_id,
            })
            .unwrap()
    };
    manager
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            power_id: 1,
            delta: 2,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();
    complete(&mut manager, 10, 31441111);
    manager
        .execute(ConduitCommand::SetSkillGroup {
            origin: ORIGIN,
            source_uid: 11,
            group: 3,
        })
        .unwrap();
    let completed = complete(&mut manager, 11, 31450131);

    assert!(matches!(
        completed.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.activation_cost == 2 && event.spent == 0 && event.power_id == 999
    ));

    manager
        .execute(ConduitCommand::SetSkillGroup {
            origin: ORIGIN,
            source_uid: 12,
            group: 3,
        })
        .unwrap();
    let repeated = complete(&mut manager, 12, 302504312);
    assert!(matches!(
        repeated.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.activation_cost == 2
    ));

    manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 0,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: -2,
        }))
        .unwrap();
    manager
        .execute(ConduitCommand::SetSkillGroup {
            origin: ORIGIN,
            source_uid: 10,
            group: 3,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31441131,
            cost_reduction: 0,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31441131,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31441131,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: 4,
        }))
        .unwrap();
    let measured_zero = manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31441131,
        })
        .unwrap();
    assert!(measured_zero.events().is_empty());

    manager.begin_round();
    let next_round = complete(&mut manager, 11, 31450131);
    assert!(next_round.events().is_empty());
}

#[test]
fn unique_skill_clears_configured_energy_pools_as_one_activation() {
    config::init(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data/excel2json")
            .to_str()
            .unwrap(),
    )
    .ok();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    for (power_id, delta) in [(0, 1), (1, 5), (2, 7)] {
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                power_id,
                delta,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
    }
    manager
        .execute(ConduitCommand::SetSkillGroup {
            origin: ORIGIN,
            source_uid: 10,
            group: 3,
        })
        .unwrap();

    assert!(manager.can_begin_skill(10, 31490151, 0));
    let began = manager
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490151,
            cost_reduction: 0,
        })
        .unwrap();
    assert!(matches!(
        began,
        ConduitChange::SkillBegan {
            power_id: 999,
            spent: 0,
            ..
        }
    ));
    manager
        .execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31490151,
        })
        .unwrap();

    let cleared = manager
        .execute(ConduitCommand::ClearPowers {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_ids: vec![0, 1, 2],
        })
        .unwrap();
    assert_eq!(manager.power(1, 0), 0);
    assert_eq!(manager.power(1, 1), 0);
    assert_eq!(manager.power(1, 2), 0);
    assert_eq!(
        manager
            .areas
            .get(&1)
            .unwrap()
            .powers
            .iter()
            .map(|power| power.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(matches!(
        cleared,
        ConduitChange::PowersCleared {
            ref spent_by_power,
            spent: 13,
            ..
        } if spent_by_power == &[(0, 1), (1, 5), (2, 7)]
    ));
    assert!(cleared.events().is_empty());
    let completed = manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31490151,
        })
        .unwrap();
    assert!(matches!(
        completed.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.spent == 13 && event.skill_id == 31490151
    ));

    manager
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490151,
            cost_reduction: 0,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31490151,
        })
        .unwrap();
    let cleared_zero = manager
        .execute(ConduitCommand::ClearPowers {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_ids: vec![0, 1, 2],
        })
        .unwrap();
    assert!(matches!(
        cleared_zero,
        ConduitChange::PowersCleared { spent: 0, .. }
    ));
    let completed_zero = manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31490151,
        })
        .unwrap();
    assert!(completed_zero.events().is_empty());
}

#[test]
fn clear_powers_without_an_activation_preserves_resources() {
    config::init(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data/excel2json")
            .to_str()
            .unwrap(),
    )
    .ok();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    manager
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            power_id: 2,
            delta: 5,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();

    assert_eq!(
        manager.execute(ConduitCommand::ClearPowers {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_ids: vec![2],
        }),
        Err(ConduitError::MissingActivation(31490151))
    );
    assert_eq!(manager.power(1, 2), 5);
    assert_eq!(manager.consumed(1, 2), 0);
}

#[test]
fn opening_reset_clears_power_and_restarts_each_device() {
    config::init(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data/excel2json")
            .to_str()
            .unwrap(),
    )
    .ok();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    manager
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            power_id: 1,
            delta: 4,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();
    manager
        .execute(ConduitCommand::StopSkill {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
        })
        .unwrap();
    assert!(!manager.can_begin_skill(10, 31490121, 0));

    let changes = manager
        .opening_reset_commands()
        .into_iter()
        .map(|command| manager.execute(command).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(manager.power(1, 1), 0);
    manager
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            power_id: 1,
            delta: 4,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();
    assert!(manager.can_begin_skill(10, 31490121, 0));
    assert!(matches!(
        changes.as_slice(),
        [
            ConduitChange::PowersReset { team: 1 },
            ConduitChange::DeviceRestarted {
                source_uid: 10,
                team: 1
            }
        ]
    ));
}

#[test]
fn action_phase_start_requires_an_existing_area() {
    let mut manager = ConduitManager::default();
    assert!(manager.action_phase_start_commands(1).is_empty());

    manager.areas.insert(
        1,
        ConduitArea {
            team: 1,
            devices: vec![
                ConduitDevice {
                    uid: 10,
                    selected_group: 1,
                    skill_groups: Vec::new(),
                },
                ConduitDevice {
                    uid: 20,
                    selected_group: 1,
                    skill_groups: Vec::new(),
                },
            ],
            powers: Vec::new(),
        },
    );

    assert_eq!(
        manager.action_phase_start_commands(1),
        vec![
            ConduitCommand::ResetPowers { team: 1 },
            ConduitCommand::RestartDevice { source_uid: 10 },
            ConduitCommand::RestartDevice { source_uid: 20 },
        ]
    );
}
