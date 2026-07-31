use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

use super::*;

#[test]
fn barcarola_resources_require_one_nonzero_configured_delta() {
    assert!(supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![3],
    )));
    assert!(!supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![0],
    )));
    assert!(!supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![1, 3],
    )));
    assert!(supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![3],
    )));
    assert!(!supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![0],
    )));
    assert!(!supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![-1],
    )));
}

#[test]
fn recover_power_and_cast_cards_consumes_only_the_casters_incantations() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                skill_group1: vec![100, 101, 102],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        },
        CardInfo {
            uid: Some(20),
            skill_id: Some(200),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(101),
            ..Default::default()
        },
    ]);
    managers.card.seed(&fight);
    let behavior = ParsedBehavior::new(
        60125,
        "RecoverPowerAndDelCardsUseSkill",
        vec![31050152, 210],
    );
    assert!(supports_recover_power_and_cast_cards(&behavior));
    assert_eq!(
        (super::super::registry::find(&behavior).unwrap().references)(&behavior).skills,
        [31050152]
    );

    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31050131,
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
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: 3,
                ..
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ConsumeForEffect(
                CardConsumeForEffect { indices, .. }
            ))),
            RuleOp::Skill(first),
            RuleOp::Skill(second),
        ] if indices == &[0, 2]
            && first.plan.skill_id == 31050152
            && first.target == SkillTarget::LogicRule(210)
            && first.mode == SkillExecutionMode::Active
            && second.plan.skill_id == 31050152
            && second.target == SkillTarget::LogicRule(210)
            && second.mode == SkillExecutionMode::Active
    ));
}

#[test]
fn add_ex_point_aggregates_fire_count_into_one_command() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
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
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20002, "AddExPoint"),
        vec![1],
        Vec::new(),
    );

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 2,
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
            ExPointCommand::Change(ExPointChange { delta: 2, .. })
        ))]
    ));
    assert_eq!(
        super::super::registry::find(&behavior)
            .unwrap()
            .fire_count_mode,
        super::super::registry::FireCountMode::Transfer
    );
}

#[test]
fn committed_card_ranks_scale_the_configured_power_gain() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243111),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243112),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243113),
            ..Default::default()
        },
    ]);
    while !managers.card.hand().is_empty() {
        managers.card.play_card(0, None, None, None).unwrap();
    }
    managers
        .execute_card(crate::engine::manager::card::CardCommand::QueueUseCard(
            crate::engine::manager::card::CardQueueUse {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(60070, "AddUseSkillCard"),
                },
                card_index: 4,
                card: sonettobuf::CardInfo {
                    uid: Some(10),
                    skill_id: Some(370001002),
                    ..Default::default()
                },
                team_type: 1,
                source_skill_id: 370001010,
                action: None,
            },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60115, "TotalSkillRankToPower", vec![3000, 4]);

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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Eureka(
            EurekaCommand::Change(EurekaChange {
                power_id: 4,
                delta: 27,
                ..
            })
        ))]
    ));
}

#[test]
fn del_ex_point_validates_and_emits_the_configured_loss() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ex_point: Some(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30001, "DelExPoint"),
        vec![1],
        vec!["1".into()],
    );

    let definition = super::super::registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 11,
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
        .as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                target_uid: 11,
                delta: -1,
                ..
            })
        ))]
    ));
}

#[test]
fn team_energy_uses_the_shared_team_gauge() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
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
    let behavior = ParsedBehavior::new(60153, "AddTeamEnergy", vec![3]);

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

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::Enable { max: None },
                ..
            })),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::ChangeValue { delta: 3 },
                ..
            }))
        ]
    ));
}

#[test]
fn emitter_energy_uses_the_enabled_inspiration_gauge() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let key =
        crate::engine::mechanic::impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    managers
        .execute_gauge(GaugeCommand::new(
            crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                key: crate::engine::skill::rule::DefinitionKey::new(0, "Test"),
            },
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60152, "AddEmitterEnergy", vec![6]);

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: crate::engine::manager::emitter::UID,
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            key: command_key,
            operation: GaugeOperation::ChangeValue { delta: 6 },
            source_uid: 10,
            ..
        }))] if *command_key == key
    ));
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

    let definition = super::super::registry::find(&behavior).unwrap();
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
    let change = managers.conduit.execute(*command).unwrap();

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

    assert!(matches!(
        managers.conduit.execute(*command).unwrap(),
        crate::engine::manager::conduit::ConduitChange::SkillGroupChanged {
            source_uid: 10,
            team: 1,
            group: 3,
            ..
        }
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
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(100034, "StopDeviceSkill", Vec::new());
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
        managers.conduit.execute(*command).unwrap(),
        crate::engine::manager::conduit::ConduitChange::SkillStopped {
            source_uid: 10,
            team: 1,
            skill_id: 31490111,
            ..
        }
    ));
}

#[test]
fn absorb_ex_point_emits_loss_then_actual_gain() {
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
        vec![5],
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
        [
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                ExPointChange {
                    target_uid: 11,
                    delta: -2,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                ExPointChange {
                    target_uid: 10,
                    delta: 2,
                    ..
                }
            )))
        ]
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
fn average_life_redistributes_team_hp_by_max_hp_ratio() {
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
    assert_eq!(values, vec![(10, 50), (11, 150)]);
}
