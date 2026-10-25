use super::*;

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
