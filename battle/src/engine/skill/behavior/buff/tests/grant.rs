use super::*;

#[test]
fn layer_copy_uses_the_buff_limit_and_preserves_main_target_overflow() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(4150001),
                    layer: Some(6),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(4150001),
                        layer: Some(28),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60068, "AddBuffByBuffLayer", vec![4150001, 4150001, 1]);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        runtime_target_uid: -1,
        ..Default::default()
    };

    let main_ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 30940132,
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
    assert_eq!(target.buff_overflow_amount, 4);
    let [RuleOp::Command(BattleCommand::Buff(command))] = main_ops.as_slice() else {
        panic!("expected one buff command");
    };
    managers.execute_buff(command.clone()).unwrap();
    assert_eq!(managers.buff.buff_id_amount(-1, 4150001), 30);

    let secondary_ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -2,
            active_skill_id: 30940132,
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
    assert_eq!(target.buff_overflow_amount, 4);
    let [RuleOp::Command(BattleCommand::Buff(command))] = secondary_ops.as_slice() else {
        panic!("expected one buff command");
    };
    assert!(matches!(
        command,
        BuffCommand::Grant(BuffGrant {
            target_uid: -2,
            buff_id: 4150001,
            amount: Some(6),
            ..
        })
    ));
}

#[test]
fn layer_range_replaces_the_selected_buff_too() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31050111),
                        layer: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31050111),
                        layer: Some(6),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(31050142),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60124,
            "AddBuffByBuffLayerRange",
        ),
        Vec::new(),
        vec![
            "31050111".into(),
            "31050141,31050142,31050143".into(),
            "5,15,25,100".into(),
        ],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

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
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                selector: BuffRemoveSelector::ExactId(31050141),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                selector: BuffRemoveSelector::ExactId(31050142),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                selector: BuffRemoveSelector::ExactId(31050143),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31050142,
                ..
            }))),
        ]
    ));
}

#[test]
fn random_pool_add_emits_distinct_configured_buff_commands() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20021, "AddBuffRanId"),
        vec![30630111, 2],
        Vec::new(),
    );
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let buff_ids = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => Some(grant.buff_id),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(buff_ids.len(), 2);
    assert_ne!(buff_ids[0], buff_ids[1]);
    assert!(buff_ids.iter().all(|id| (30630112..=30630115).contains(id)));
}

#[test]
fn ordinary_add_buff_rule_executes_without_a_battle_intent() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(1, "AddBuff"),
        vec![101, 2],
        Vec::new(),
    );
    let mut managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
        panic!("expected one buff command");
    };
    let changes = managers.execute_buff(command.clone()).unwrap();

    assert!(matches!(
        changes.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::BuffAdded(event)]
            if event.source_uid == 10
                && event.target_uid == 10
                && event.buff_id == 101
                && event.after_amount == 2
    ));
}

#[test]
fn enemy_burn_conversion_uses_the_settlement_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let event = crate::engine::event::payload::BattleEvent::BuffsSettled(vec![
        crate::engine::event::payload::BuffChangeEvent {
            source_uid: 10,
            target_uid: -1,
            buff_uid: 20,
            buff_id: 4150001,
            before_amount: 11,
            after_amount: 5,
            act_id: 0,
            act_value: 0,
        },
    ]);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            50035,
            "AddBuffBasedOnEnemyBurnUseCount",
        ),
        vec![30810108, 1000],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: Some(&event),
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
        [RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Accumulate(BuffGrant {
                buff_id: 30810108,
                amount: Some(6),
                ..
            })
        ))]
    ));
}

#[test]
fn add_buff_round_extends_existing_type_family_without_granting_a_new_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31080132),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20005, "AddBuffRound"),
        vec![31080131, 2],
        Vec::new(),
    );
    let command = change_duration_command(10, &behavior, BuffSelector::IdOrType).unwrap();
    let BuffCommand::ChangeDuration(change) = &command else {
        panic!("expected one duration change");
    };

    assert_eq!(change.selector, BuffSelector::IdOrType(31080131));
    assert_eq!(
        change.origin.key,
        crate::engine::skill::rule::DefinitionKey::new(20005, "AddBuffRound")
    );
    let changes = managers.execute_buff(command).unwrap();
    assert!(changes.change.added.is_none());
    assert_eq!(changes.change.refreshed.len(), 1);
    assert_eq!(changes.change.refreshed[0].before.duration, Some(1));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(3));
}

#[test]
fn add_buff_duration_updates_owned_instances_through_one_manager_command() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31080131),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60145, "AddBuffDuration"),
        vec![31080131, 5],
        Vec::new(),
    );
    let command = change_duration_command(10, &behavior, BuffSelector::ExactId).unwrap();

    let changes = managers.execute_buff(command).unwrap();

    assert_eq!(changes.change.refreshed.len(), 1);
    assert_eq!(changes.change.refreshed[0].before.duration, Some(1));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(6));
}

#[test]
fn consume_buff_by_type_id_emits_one_manager_owned_consume() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(50014, "ConsumeBuffByTypeId"),
        vec![530000111, 1],
        Vec::new(),
    );

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 530000151,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                target_uid: 20,
                selector: BuffSelector::IdOrType(530000111),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            }
        )))]
    ));
}

#[test]
fn consume_buff_by_type_id_2_uses_an_exact_type_selector() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(50016, "ConsumeBuffByTypeId2"),
        vec![90001, 1],
        Vec::new(),
    );

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 22302341,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                target_uid: 20,
                selector: BuffSelector::TypeId(90001),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            }
        )))]
    ));
}

#[test]
fn damage_window_removes_each_configured_buff_id_or_type() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60010, "DisperseForce2"),
        vec![31140111, 31140112],
        vec!["31140111,31140112".into()],
    );

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 31140131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
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
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 20,
                selector: BuffRemoveSelector::IdOrType(31140111),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 20,
                selector: BuffRemoveSelector::IdOrType(31140112),
                ..
            })))
        ]
    ));
}

#[test]
fn consume_card_add_buff_emits_card_consumption_before_the_grant() {
    let mut managers = BattleManagers::default();
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31280121),
            card_effect: Some(1),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(20),
            skill_id: Some(999),
            card_effect: Some(3),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31280122),
            card_effect: Some(2),
            ..Default::default()
        },
    ]);
    managers.card.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                skill_group2: vec![31280121, 31280122, 31280123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60222, "ConsumeCardAddBuff"),
        vec![31280113, 10],
        vec!["31280113".into(), "10,15,25".into()],
    );

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31280131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
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
            RuleOp::Command(BattleCommand::Card(CardCommand::ConsumeForEffect(
                crate::engine::manager::card::CardConsumeForEffect {
                    owner_uid: 10,
                    indices,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31280113,
                amount: Some(25),
                occurrences: 1,
                child_uid_reservations: 0,
                ..
            })))
        ] if indices == &[0, 2]
    ));
}
