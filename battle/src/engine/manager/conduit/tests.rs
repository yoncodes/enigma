use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{Fight, FightDeviceAreaInfo, FightEntityInfo, FightTeam};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60291, "AddDevicePower"),
};

#[test]
fn parses_configured_skill_group_without_losing_cost_identity() {
    crate::test_support::init_config();
    let groups = crate::catalog::configured_conduit_device(
        crate::test_support::game_data(),
        &FightEntityInfo {
            model_id: Some(3149),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        groups[0],
        vec![
            ConduitSkill {
                skill_id: 31490111,
                cost_type: 1,
                cost_value: 0,
                is_stopped: false,
            },
            ConduitSkill {
                skill_id: 31490121,
                cost_type: 1,
                cost_value: 3,
                is_stopped: false,
            },
        ]
    );
}

#[test]
fn configured_seed_matches_the_borrowed_database_adapter() {
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

    let configured = ConduitManager::configured(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &fight,
    );
    let legacy = ConduitManager::seed_with_game_data(crate::test_support::game_data(), &fight);

    assert_eq!(configured.areas, legacy.areas);
    assert_eq!(
        configured.initialization_errors,
        legacy.initialization_errors
    );
}

#[test]
fn configured_seed_preserves_an_empty_captured_area() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let manager = ConduitManager::configured(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &fight,
    );

    assert_eq!(
        manager.action_phase_start_commands(1),
        vec![ConduitCommand::ResetPowers { team: 1 }]
    );
    assert!(manager.initialization_commands().is_empty());
}

#[test]
fn configured_seed_uses_selected_destiny_device_from_entity_loadout() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3025),
                ex_skill_level: Some(0),
                destiny_stone: Some(302502),
                destiny_rank: Some(4),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let manager = ConduitManager::configured(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &fight,
    );

    assert_eq!(
        manager
            .selected_skills(10)
            .unwrap()
            .into_iter()
            .map(|skill| skill.skill_id)
            .collect::<Vec<_>>(),
        vec![302503112]
    );
    assert_eq!(
        manager.skill_ids().collect::<Vec<_>>(),
        vec![302503112, 302503212, 302504312]
    );
}

#[test]
fn terminal_roster_refresh_removes_defeated_devices_and_preserves_live_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3144),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ConduitManager::seed(&fight);
    manager
        .execute(ConduitCommand::SelectGroup {
            source_uid: 10,
            group: 2,
        })
        .unwrap();
    let stopped_skill = manager.selected_skills(10).unwrap()[0].skill_id;
    manager
        .execute(ConduitCommand::StopSkill {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: stopped_skill,
        })
        .unwrap();
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

    let commands = manager.terminal_roster_refresh_commands(&[11]);
    assert_eq!(commands.len(), 1);
    let change = manager.execute(commands[0].clone()).unwrap();
    let ConduitChange::RosterRefreshed(area) = change else {
        panic!("expected a Conduit roster refresh");
    };
    assert_eq!(area.devices.len(), 1);
    assert_eq!(area.devices[0].uid, 10);
    assert_eq!(area.devices[0].selected_group, 2);
    assert!(
        area.devices[0]
            .skill_groups
            .iter()
            .flatten()
            .any(|skill| skill.skill_id == stopped_skill && skill.is_stopped)
    );
    assert_eq!(
        area.powers
            .iter()
            .find(|power| power.id == 1)
            .unwrap()
            .value,
        4
    );
    assert!(manager.terminal_roster_refresh_commands(&[11]).is_empty());
}

#[test]
fn selects_the_requested_group_on_the_owning_device() {
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

    assert_eq!(
        manager
            .execute(ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            })
            .unwrap(),
        ConduitChange::GroupSelected {
            source_uid: 10,
            team: 1,
            group: 2,
        }
    );
}

#[test]
fn device_skill_spends_its_configured_power_and_updates_round_counters() {
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
            kind: ConduitPowerChangeKind::Interval,
        }))
        .unwrap();

    manager
        .execute(ConduitCommand::BeginSkill {
            source_uid: 10,
            skill_id: 31490121,
            cost_reduction: 0,
        })
        .unwrap();
    let committed = manager
        .execute(ConduitCommand::CommitSkillCost {
            source_uid: 10,
            skill_id: 31490121,
        })
        .unwrap();
    assert!(matches!(
        committed,
        ConduitChange::SkillCostCommitted {
            power_id: 1,
            activation_cost: 3,
            consumed_this_round: 3,
            ..
        }
    ));
    manager
        .execute(ConduitCommand::CompleteActivation {
            source_uid: 10,
            skill_id: 31490121,
        })
        .unwrap();
    manager
        .execute(ConduitCommand::FinishSkill {
            source_uid: 10,
            skill_id: 31490121,
        })
        .unwrap();

    assert_eq!(manager.power(1, 1), 1);
    assert_eq!(manager.consumed(1, 1), 3);
    assert_eq!(manager.uses(10), 1);
}

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
