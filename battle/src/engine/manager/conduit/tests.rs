use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

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
fn configured_seed_uses_selected_destiny_device_from_entity_loadout() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3025),
                ex_skill_level: Some(2),
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
        vec![302524112]
    );
    assert_eq!(
        manager.skill_ids().collect::<Vec<_>>(),
        vec![302524112, 302514212, 302504312]
    );
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
fn explicit_and_normal_changes_share_team_round_counters_across_devices() {
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
    assert_eq!(manager.consumed_for_skill(10, 31440111), Some(7));
    assert_eq!(manager.consumed_for_skill(11, 302524112), Some(7));

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
fn unique_skill_clears_both_energy_pools_as_one_activation() {
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
    for (power_id, delta) in [(1, 5), (2, 7)] {
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

    let cleared = manager
        .execute(ConduitCommand::ClearPowers {
            origin: ORIGIN,
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_ids: [1, 2],
        })
        .unwrap();
    assert_eq!(manager.power(1, 1), 0);
    assert_eq!(manager.power(1, 2), 0);
    assert!(matches!(
        cleared,
        ConduitChange::PowersCleared { spent: 12, .. }
    ));
    assert!(matches!(
        cleared.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
            if event.spent == 12 && event.skill_id == 31490151
    ));
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
fn action_phase_start_commands_require_an_existing_area() {
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
