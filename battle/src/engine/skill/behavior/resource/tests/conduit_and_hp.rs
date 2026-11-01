use super::*;
use crate::engine::event::kind::EventKind;

#[test]
fn exact_conduit_counter_behavior_commits_typed_round_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    for (kind, delta, expected) in [
        (2, 2, ConduitCounterKind::Activation),
        (1, 4, ConduitCounterKind::EnergyAccumulation),
        (1, 6, ConduitCounterKind::EnergyAccumulation),
    ] {
        let behavior = ParsedBehavior::new(60297, "AddDeviceCounter", vec![kind, delta]);
        let definition = super::super::super::registry::find(&behavior).unwrap();
        assert_eq!(definition.kind, BehaviorKind::AddConduitCounter);
        assert_eq!(definition.output_owner, OutputOwner::CausingEvent);
        assert_eq!(
            definition.output_owner.resolve(None, false),
            OutputOwner::Skill
        );
        assert_eq!(
            definition
                .output_owner
                .resolve(Some(EventKind::SkillAction), false),
            OutputOwner::Parent
        );
        assert!(
            definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
        let ops = rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31447002,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();
        let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
            panic!("expected one Conduit counter command");
        };
        let change = managers.conduit.execute(command.clone()).unwrap();
        assert!(matches!(
            change,
            crate::engine::manager::conduit::ConduitChange::CounterChanged {
                kind: actual,
                requested_delta,
                ..
            } if actual == expected && requested_delta == delta
        ));
    }

    for args in [
        vec![],
        vec![1],
        vec![0, 2],
        vec![3, 2],
        vec![1, 0],
        vec![2, -1],
        vec![1, 2, 3],
    ] {
        let behavior = ParsedBehavior::new(60297, "AddDeviceCounter", args);
        let definition = super::super::super::registry::find(&behavior).unwrap();
        assert!(
            !definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
    }
}

#[test]
fn exact_conduit_power_behavior_commits_typed_energy() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60291, "AddDevicePower", vec![1, 4, 1]);

    let definition = super::super::super::registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490111,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
        panic!("expected one Conduit command");
    };
    let change = managers.conduit.execute(command.clone()).unwrap();

    assert_eq!(managers.conduit.power(1, 1), 4);
    assert!(matches!(
        change,
        crate::engine::manager::conduit::ConduitChange::PowerChanged {
            power_id: 1,
            applied_delta: 4,
            kind: ConduitPowerChangeKind::Interval,
            ..
        }
    ));
}

#[test]
fn exact_conduit_ex_point_behavior_uses_the_entity_resource_owner() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ex_point_type: Some(4),
                ex_point_max: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60292, "AddDeviceExPoint", vec![8]);

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490111,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                target_uid: 10,
                delta: 8,
                effect_type,
                config_effect,
                origin,
                ..
            })
        ))] if *effect_type == EffectType::Expointchange as i32
            && *config_effect == 0
            && origin.key == crate::engine::skill::rule::DefinitionKey::new(
                60292,
                "AddDeviceExPoint",
            )
    ));
}

#[test]
fn exact_conduit_group_behavior_changes_the_manager_owned_selection() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60293, "SetDeviceSkillIndex", vec![3]);
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490161,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
        panic!("expected one Conduit command");
    };

    let change = managers.conduit.execute(command.clone()).unwrap();
    assert!(matches!(
        change,
        crate::engine::manager::conduit::ConduitChange::SkillGroupChanged {
            origin,
            source_uid: 10,
            team: 1,
            group: 3,
            ..
        } if origin.key == crate::engine::skill::rule::DefinitionKey::new(
            60293,
            "SetDeviceSkillIndex",
        )
    ));
    assert_eq!(managers.conduit.selected_group(10), Some(3));
}

#[test]
fn interval_conduit_skill_stops_the_exact_active_skill() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    for opcode in [100034, 60294] {
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .conduit
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(opcode, "StopDeviceSkill"),
                },
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 3,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::new(opcode, "StopDeviceSkill", Vec::new());
        let ops = rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31490111,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();
        let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
            panic!("expected one Conduit command");
        };

        assert!(matches!(
            managers.conduit.execute(command.clone()).unwrap(),
            crate::engine::manager::conduit::ConduitChange::SkillStopped {
                origin,
                source_uid: 10,
                team: 1,
                skill_id: 31490111,
            } if origin.key == crate::engine::skill::rule::DefinitionKey::new(
                opcode,
                "StopDeviceSkill",
            )
        ));
        assert!(!managers.conduit.can_begin_skill(10, 31490111, 0));
        assert!(managers.conduit.can_begin_skill(10, 31490121, 0));
        assert!(matches!(
            managers.conduit.execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490111,
                cost_reduction: 0,
            }),
            Err(crate::engine::manager::conduit::ConduitError::StoppedSkill(
                31490111
            ))
        ));

        managers
            .conduit
            .execute(ConduitCommand::RestartDevice { source_uid: 10 })
            .unwrap();
        assert!(managers.conduit.can_begin_skill(10, 31490111, 0));
    }
}

#[test]
fn team_action_restarts_a_device_only_after_its_skill_stops() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3145),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31450142),
                    from_uid: Some(10),
                    layer: Some(4),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60308, "ConsumeBuffResetDevice", vec![31450142, 4]);
    let other_device = crate::engine::event::payload::BattleEvent::AllyAction(
        crate::engine::skill::action::ActionEvent {
            source_uid: 11,
            mode: SkillExecutionMode::Device,
            is_interval: true,
            ..Default::default()
        },
    );
    let owner_active = crate::engine::event::payload::BattleEvent::AllyAction(
        crate::engine::skill::action::ActionEvent {
            source_uid: 10,
            mode: SkillExecutionMode::Active,
            is_interval: true,
            ..Default::default()
        },
    );
    let owner_non_interval = crate::engine::event::payload::BattleEvent::AllyAction(
        crate::engine::skill::action::ActionEvent {
            source_uid: 10,
            mode: SkillExecutionMode::Device,
            ..Default::default()
        },
    );
    let owner_interval = crate::engine::event::payload::BattleEvent::AllyAction(
        crate::engine::skill::action::ActionEvent {
            source_uid: 10,
            mode: SkillExecutionMode::Device,
            is_interval: true,
            ..Default::default()
        },
    );
    let mut ops_for = |managers: &BattleManagers, event| {
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31450141,
                transfer_count: 1,
                event,
                managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
    };

    assert_eq!(ops_for(&managers, Some(&other_device)), Some(Vec::new()));
    managers
        .conduit
        .execute(ConduitCommand::StopSkill {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60294, "StopDeviceSkill"),
            },
            source_uid: 10,
            team: 1,
            skill_id: 31450111,
        })
        .unwrap();
    for event in [&owner_active, &owner_non_interval, &owner_interval] {
        let ops = ops_for(&managers, Some(event)).unwrap();
        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                    origin: _,
                    target_uid: 10,
                    selector: BuffSelector::IdOrType(31450142),
                    amount: 4,
                    depleted: DepletedBuff::RemoveWithZeroLayer,
                }))),
                RuleOp::Command(BattleCommand::Conduit(ConduitCommand::RestartDevice {
                    source_uid: 10,
                })),
            ]
        ));
    }
    assert!(matches!(
        ops_for(&managers, Some(&other_device)).unwrap().as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_))),
            RuleOp::Command(BattleCommand::Conduit(ConduitCommand::RestartDevice {
                source_uid: 10,
            })),
        ]
    ));
    assert!(
        ops_for(&managers, None).is_none(),
        "event-free variants remain unsupported"
    );
}

#[test]
fn absorb_ex_point_emits_transfer() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ex_point: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ex_point: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30011, "AbsorbExPoint"),
        vec![5, 0],
        Vec::new(),
    );

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &crate::engine::skill::target::TargetPool::from_fight(&fight),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Transfer(ExPointTransfer {
                source_uid: 10,
                from_uid: 11,
                to_uid: 10,
                amount: 5,
                ..
            })
        ))]
    ));
}

#[test]
fn crit_power_progress_counts_a_critical_incantation_once_per_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(0),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60187, "AddPowerByCritCount", vec![2, 1]);

    for expected in [0, 1] {
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext {
            action_crit_count: 3,
            ..Default::default()
        };
        let op = rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 0,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap()
        .pop()
        .unwrap();
        let RuleOp::Command(BattleCommand::Eureka(command)) = op else {
            panic!("expected progress-gated Eureka command")
        };

        managers.execute_eureka(command).unwrap();
        assert_eq!(
            managers.eureka.get(10, EUREKA_RESOURCE_ID).current,
            expected
        );
    }

    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        critical_action_count: 6,
        ..Default::default()
    };
    let op = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap()
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Eureka(command)) = op else {
        panic!("expected progress-gated Eureka command")
    };
    managers.execute_eureka(command).unwrap();
    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 4);
}

#[test]
fn average_life_uses_the_mean_of_each_ally_hp_ratio() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(300),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20011, "AverageLife"),
        vec![0],
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    let values = ops
        .into_iter()
        .map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::SetCurrent(set))) => {
                (set.target_uid, set.value)
            }
            _ => panic!("expected current-HP set command"),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec![(10, 66), (11, 200)]);
}

#[test]
fn average_life_preserves_capture_precision_across_unequal_max_hp() {
    assert_eq!(
        super::super::average_life_values(&[
            (1, 8016, 12730),
            (2, 8302, 13310),
            (3, 6413, 11274),
            (4, 7982, 12990),
        ]),
        Some(vec![(1, 7754), (2, 8108), (3, 6867), (4, 7913)])
    );
}
