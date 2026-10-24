use super::*;

#[test]
fn zero_cost_device_skill_registers_its_power_pool() {
    crate::test_support::init_config();
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
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490111,
            cost_reduction: 0,
        })
        .unwrap();

    assert_eq!(
        manager
            .areas
            .get(&1)
            .unwrap()
            .powers
            .iter()
            .map(|power| (power.id, power.value))
            .collect::<Vec<_>>(),
        vec![(1, 0)]
    );
}

#[test]
fn device_skill_uses_generic_power_for_the_typed_shortfall() {
    crate::test_support::init_config();
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
    for (power_id, delta) in [(1, 2), (0, 1)] {
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

    assert!(manager.can_begin_skill(10, 31490121, 0));
    assert!(matches!(
        manager
            .execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490121,
                cost_reduction: 0,
            })
            .unwrap(),
        ConduitChange::SkillBegan {
            spent: 3,
            generic_spent: 1,
            ..
        }
    ));
    assert_eq!(manager.power(1, 1), 0);
    assert_eq!(manager.power(1, 0), 0);
}

#[test]
fn finish_skill_uses_configured_ownership_after_group_switch() {
    crate::test_support::init_config();
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
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490111,
            cost_reduction: 0,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::SetSkillGroup {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60293, "SetDeviceSkillIndex"),
            },
            source_uid: 10,
            group: 2,
        })
        .unwrap();

    let finished = manager
        .execute(ConduitCommand::FinishSkill {
            source_uid: 10,
            skill_id: 31490111,
        })
        .unwrap();

    assert!(matches!(
        finished,
        ConduitChange::SkillFinished {
            source_uid: 10,
            team: 1,
            skill_id: 31490111,
            uses_this_round: 1,
        }
    ));
    assert_eq!(manager.uses(10), 1);
    assert_eq!(manager.counter(1, ConduitCounterKind::Activation), 1);
}

#[test]
fn power_consumption_stays_separate_from_explicit_team_counters() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3025),
                    ex_skill_level: Some(2),
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60297, "AddDeviceCounter"),
    };

    let explicit_activation = manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin,
            source_uid: 11,
            team: 1,
            skill_id: 0,
            kind: ConduitCounterKind::Activation,
            delta: 2,
        }))
        .unwrap();
    assert!(matches!(
        explicit_activation,
        ConduitChange::CounterChanged {
            kind: ConduitCounterKind::Activation,
            after: 2,
            ..
        }
    ));

    for (source_uid, skill_id, power_id, cost) in [(10, 31440111, 1, 2), (11, 302524112, 2, 1)] {
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid,
                team: 1,
                power_id,
                delta: cost,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
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
        let finished = manager
            .execute(ConduitCommand::FinishSkill {
                source_uid,
                skill_id,
            })
            .unwrap();
        assert!(matches!(
            finished,
            ConduitChange::SkillFinished {
                uses_this_round,
                ..
            } if uses_this_round == if source_uid == 10 { 3 } else { 4 }
        ));
        manager
            .execute(ConduitCommand::CompleteActivation {
                source_uid,
                skill_id,
            })
            .unwrap();
    }
    assert_eq!(manager.uses(10), 1);
    assert_eq!(manager.uses(11), 1);
    assert_eq!(manager.counter(1, ConduitCounterKind::Activation), 4);

    let explicit_energy = manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin,
            source_uid: 10,
            team: 1,
            skill_id: 0,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: 4,
        }))
        .unwrap();
    assert!(matches!(
        explicit_energy,
        ConduitChange::CounterChanged {
            kind: ConduitCounterKind::EnergyAccumulation,
            after: 7,
            ..
        }
    ));
    assert_eq!(manager.consumed_for_skill(10, 31440111), Some(2));
    assert_eq!(manager.consumed_for_skill(11, 302524112), Some(1));

    manager.begin_round();
    assert_eq!(manager.uses(10), 0);
    assert_eq!(manager.consumed_for_skill(10, 31440111), Some(0));
    assert_eq!(manager.consumed(1, 1), 0);
    assert_eq!(manager.consumed(1, 2), 0);
}

#[test]
fn reduced_spend_keeps_the_configured_activation_cost() {
    crate::test_support::init_config();
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
            delta: 3,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();
    manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 0,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: 5,
        }))
        .unwrap();

    manager
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490121,
            cost_reduction: 1,
        })
        .unwrap();
    assert_eq!(
        manager.execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490121,
            cost_reduction: 1,
        }),
        Err(ConduitError::ActivationInProgress(31490121))
    );
    assert_eq!(
        manager.execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31490121,
        }),
        Err(ConduitError::ActivationNotCommitted(31490121))
    );
    manager
        .execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31490121,
        })
        .unwrap();
    assert_eq!(
        manager.execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31490121,
        }),
        Err(ConduitError::ActivationAlreadyCommitted(31490121))
    );
    manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: 1,
        }))
        .unwrap();
    let change = manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31490121,
        })
        .unwrap();

    assert_eq!(manager.power(1, 1), 1);
    assert_eq!(manager.consumed(1, 1), 3);
    assert!(matches!(
        change.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.activation_cost == 3 && event.spent == 2
    ));
}

#[test]
fn unique_activation_publishes_the_current_cost_without_committing_it_again() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                ex_skill_level: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    manager
        .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 0,
            kind: ConduitCounterKind::EnergyAccumulation,
            delta: 3,
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
    assert_eq!(
        manager.counter(1, ConduitCounterKind::EnergyAccumulation),
        7
    );
    let change = manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31441131,
        })
        .unwrap();

    assert!(matches!(
        change.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.activation_cost == 3 && event.spent == 0
    ));
}
