use super::*;

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
        (super::super::super::registry::find(&behavior)
            .unwrap()
            .references)(&behavior)
        .skills,
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
            && first.target_observed_extra_action == Some(false)
            && first.action_hand_settlement.is_some()
            && second.plan.skill_id == 31050152
            && second.target == SkillTarget::LogicRule(210)
            && second.mode == SkillExecutionMode::Active
            && second.target_observed_extra_action == Some(false)
            && second.action_hand_settlement.is_none()
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
        super::super::super::registry::find(&behavior)
            .unwrap()
            .fire_count_mode,
        super::super::super::registry::FireCountMode::Transfer
    );
}

#[test]
fn add_ex_point_does_not_write_a_special_resource() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ex_point_type: Some(ExPointKind::Faith.as_wire()),
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
        vec![5],
        Vec::new(),
    );

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

    assert!(ops.is_empty());
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

    let definition = super::super::super::registry::find(&behavior).unwrap();
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
            ExPointCommand::Reduce(ExPointChange {
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

fn per_type_buff_energy_ops(fight: &Fight) -> (Vec<RuleOp>, i32, i32) {
    let mut managers = BattleManagers::seeded(fight);
    let key = crate::engine::mechanic::impromptu::team_energy_key(1);
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
    let before = managers.buff.buff_id_amount(10, 303901411);
    let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60264,
            "PerTypeBuffAddEnergyToTeam",
        ),
        Vec::new(),
        vec!["303901411".to_owned(), "1".to_owned(), "1".to_owned()],
    );
    let ops = super::super::super::rule_ops(
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
    .expect("exact per-type buff team-energy behavior must emit");
    let after = managers.buff.buff_id_amount(10, 303901411);
    (ops, before, after)
}

#[test]
fn per_type_buff_team_energy_counts_exact_layers_for_the_source_team() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2001),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(4),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2002),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    let (ops, before, after) = per_type_buff_energy_ops(&fight);
    let [RuleOp::Command(BattleCommand::Gauge(command))] = ops.as_slice() else {
        panic!("expected one aggregate team-energy command")
    };

    assert_eq!(before, 7);
    assert_eq!(after, 7);
    assert_eq!(
        command.key,
        crate::engine::mechanic::impromptu::team_energy_key(1)
    );
    assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 7 });
    assert_eq!(command.source_uid, 10);
    assert_eq!(command.config_effect, 60264);
}

#[test]
fn per_type_buff_team_energy_is_a_noop_without_the_exact_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2004),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    let (ops, before, after) = per_type_buff_energy_ops(&fight);
    assert!(ops.is_empty());
    assert_eq!(before, 0);
    assert_eq!(after, 0);
}

fn per_type_buff_emitter_energy_ops(
    fight: &Fight,
    target_uid: i64,
    enable_gauge: bool,
) -> (Vec<RuleOp>, i32, i32) {
    let mut managers = BattleManagers::seeded(fight);
    let key =
        crate::engine::mechanic::impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    if enable_gauge {
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
    }
    let before = managers.buff.buff_id_amount(10, 303901411);
    let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60266,
            "PerTypeBuffAddEnergyToEmitter",
        ),
        Vec::new(),
        vec!["303901411".to_owned(), "2".to_owned()],
    );
    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid,
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
    .expect("exact per-type emitter-energy behavior must emit");
    let after = managers.buff.buff_id_amount(10, 303901411);
    (ops, before, after)
}

#[test]
fn per_type_buff_emitter_energy_counts_source_layers_for_any_target_without_consuming_them() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2001),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(4),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2002),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    for target_uid in [10, crate::engine::manager::emitter::UID] {
        let (ops, before, after) = per_type_buff_emitter_energy_ops(&fight, target_uid, true);
        let [RuleOp::Command(BattleCommand::Gauge(command))] = ops.as_slice() else {
            panic!("expected one aggregate emitter-energy command")
        };

        assert_eq!(before, 7);
        assert_eq!(after, 7);
        assert_eq!(
            command.key,
            crate::engine::mechanic::impromptu::inspiration_key(
                crate::engine::manager::emitter::UID,
            )
        );
        assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 14 });
        assert_eq!(command.source_uid, 10);
        assert_eq!(command.config_effect, 60266);
    }
}

#[test]
fn per_type_buff_emitter_energy_is_a_noop_without_layers_or_gauge() {
    crate::test_support::init_config();
    let no_layers = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2004),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };
    let (ops, before, after) = per_type_buff_emitter_energy_ops(&no_layers, 10, true);
    assert!(ops.is_empty());
    assert_eq!((before, after), (0, 0));

    let with_layers = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2001),
                    buff_id: Some(303901411),
                    from_uid: Some(10),
                    layer: Some(7),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (ops, before, after) = per_type_buff_emitter_energy_ops(&with_layers, 10, false);
    assert!(ops.is_empty());
    assert_eq!((before, after), (7, 7));
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
