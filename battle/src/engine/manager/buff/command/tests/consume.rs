use super::*;

#[test]
fn id_or_type_consume_plans_update_then_depletion_removal() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(30631),
                    uid: Some(2),
                    count: Some(3),
                    layer: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Condition,
        key: DefinitionKey::new(19004, "HasBuffId"),
    };
    let consume = |amount, depleted| {
        BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: 10,
            selector: BuffSelector::IdOrType(8178),
            amount,
            depleted,
        })
    };

    let kept = manager
        .execute(&HpManager::default(), consume(1, DepletedBuff::Keep))
        .unwrap();
    assert_eq!(kept.change.refreshed[0].after.count, Some(2));
    assert!(matches!(
        kept.events().as_slice(),
        [BattleEvent::BuffChanged(event)]
            if event.buff_id == 30631
                && event.before_amount == 3
                && event.after_amount == 2
    ));

    let removed = manager
        .execute(&HpManager::default(), consume(2, DepletedBuff::Remove))
        .unwrap();
    assert_eq!(removed.change.removed.len(), 1);
    assert_eq!(removed.change.removed[0].before_amount, 2);
    assert_eq!(removed.change.removed[0].buff.count, Some(0));
    assert!(matches!(
        removed.events().as_slice(),
        [BattleEvent::BuffRemoved(event)]
            if event.buff_uid == 2
                && event.buff_id == 30631
                && event.before_amount == 2
                && event.after_amount == 0
    ));
    assert!(!manager.has_buff_id(10, 30631));
}

#[test]
fn layered_consume_publishes_only_the_resulting_amount() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31260141),
                    uid: Some(20),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Consume(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1024, "MonitorContinueChannel"),
                },
                target_uid: 10,
                selector: BuffSelector::Uid(20),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )
        .unwrap();

    assert_eq!(changes.change.refreshed[0].before.layer, Some(2));
    assert_eq!(changes.change.refreshed[0].after.layer, Some(1));
    assert!(!changes.refresh_wire[0].echo_before);
}

#[test]
fn depleted_stacked_consume_reports_zero_in_the_delete_snapshot() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31250191),
                    uid: Some(20),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Consume(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(50014, "ConsumeBuffByTypeId"),
                },
                target_uid: 10,
                selector: BuffSelector::IdOrType(31250191),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )
        .unwrap();

    assert!(manager.snapshot(10, 20).is_none());
    assert_eq!(changes.change.removed[0].before_amount, 1);
    assert_eq!(changes.change.removed[0].buff.layer, Some(0));
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::BuffRemoved(event)]
            if event.before_amount == 1 && event.after_amount == 0
    ));
}

#[test]
fn depleted_layer_consume_reports_zero_in_the_delete_snapshot() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31260141),
                    uid: Some(20),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Consume(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1024, "MonitorContinueChannel"),
                },
                target_uid: 10,
                selector: BuffSelector::Uid(20),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )
        .unwrap();

    assert!(manager.snapshot(10, 20).is_none());
    assert_eq!(changes.change.removed[0].before_amount, 1);
    assert_eq!(changes.change.removed[0].buff.layer, Some(0));
}

#[test]
fn uid_consume_removes_only_the_selected_instance() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(30631),
                        uid: Some(2),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(30631),
                        uid: Some(3),
                        count: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Consume(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(113, "AttrOnlyCalDamageAttack"),
                },
                target_uid: 10,
                selector: BuffSelector::Uid(3),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )
        .unwrap();

    assert!(manager.snapshot(10, 2).is_some());
    assert!(manager.snapshot(10, 3).is_none());
    assert_eq!(changes.change.removed[0].buff.uid, Some(3));
    assert_eq!(changes.change.removed[0].buff.count, Some(0));
}

#[test]
fn explicit_count_consume_preserves_a_layer_stacked_buffs_layer() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(4150002),
                    uid: Some(2),
                    count: Some(1),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::ConsumeCount(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(748, "UseDamageSkillAddToTarget"),
                },
                target_uid: 10,
                selector: BuffSelector::Uid(2),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )
        .unwrap();

    assert!(manager.snapshot(10, 2).is_none());
    assert_eq!(changes.change.removed[0].buff.layer, Some(3));
    assert_eq!(changes.change.removed[0].buff.count, Some(0));
    assert_eq!(changes.change.removed[0].config_effect, 0);
}

#[test]
fn one_counted_grant_emits_refresh_wire_markers_once_for_the_transaction() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31020111,
                amount: Some(5),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert!(changes.change.added.is_some());
    assert_eq!(changes.change.refreshed.len(), 4);
    assert!(
        changes
            .refresh_wire
            .iter()
            .all(|wire| wire.markers.is_empty())
    );
    assert_eq!(
        crate::engine::packet::effect::EffectPacket::recorded_buff_changes(&changes)
            .into_iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Addtotarget as i32),
            Some(sonettobuf::effect_type_enum::EffectType::None as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32),
        ]
    );
}

#[test]
fn coalesced_consume_plans_across_distinct_instances_atomically() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(30631),
                        uid: Some(2),
                        count: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(30631),
                        uid: Some(3),
                        count: Some(3),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let command = BuffCommand::ConsumeCoalesced(BuffConsume {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60035, "ConsumeBuffAttrFix"),
        },
        target_uid: 10,
        selector: BuffSelector::IdOrType(8178),
        amount: 4,
        depleted: DepletedBuff::Remove,
    });

    let plan = manager.plan(&HpManager::default(), command).unwrap();
    assert_eq!(manager.active_for(10).count(), 2);
    let changes = manager.commit(&HpManager::default(), plan);

    assert_eq!(changes.change.removed[0].buff.uid, Some(2));
    assert_eq!(changes.change.refreshed[0].after.uid, Some(3));
    assert_eq!(changes.change.refreshed[0].after.count, Some(1));
}

#[test]
fn exact_uid_commands_do_not_collapse_duplicate_buff_ids() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(20),
                        uid: Some(2),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(20),
                        uid: Some(3),
                        from_uid: Some(11),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(20),
                        uid: Some(4),
                        from_uid: Some(12),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(503, "AddToTarget"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(3),
            }),
        )
        .unwrap();

    assert_eq!(changes.change.removed[0].buff.uid, Some(3));
    assert_eq!(changes.change.removed[0].config_effect, 503);
    assert!(manager.snapshot(10, 2).is_some());
    assert!(manager.snapshot(10, 3).is_none());
    assert!(manager.snapshot(10, 4).is_some());

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetAmount(BuffSetAmount {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1048, "Radiance"),
                },
                target_uid: 10,
                buff_uid: 2,
                amount: BuffAmount::Layer(4),
            }),
        )
        .unwrap();
    assert_eq!(changes.change.refreshed[0].after.layer, Some(4));
    assert_eq!(manager.snapshot(10, 2).unwrap().layer, Some(4));

    manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetAmount(BuffSetAmount {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1026, "CreateMaxHpAdditionalDamageAndRemove"),
                },
                target_uid: 10,
                buff_uid: 2,
                amount: BuffAmount::Count(3),
            }),
        )
        .unwrap();
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetAmount(BuffSetAmount {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1026, "CreateMaxHpAdditionalDamageAndRemove"),
                },
                target_uid: 10,
                buff_uid: 2,
                amount: BuffAmount::Count(1),
            }),
        )
        .unwrap();
    assert_eq!(manager.snapshot(10, 2).unwrap().count, Some(1));
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::BuffChanged(event)]
            if event.before_amount == 3 && event.after_amount == 1
    ));

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetState(BuffSetState {
                ex_info: None,
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(806, "ExPointOverflowBank"),
                },
                target_uid: 10,
                buff_uid: 2,
                params: Some("806#3".to_owned()),
                act_info: Some(vec![BuffActInfo {
                    act_id: Some(806),
                    param: vec![3],
                    str_param: Some(String::new()),
                }]),
            }),
        )
        .unwrap();
    assert_eq!(
        manager
            .snapshot(10, 2)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("806#3")
    );
    assert_eq!(manager.snapshot(10, 2).unwrap().act_info[0].param, vec![3]);
    assert!(changes.events().is_empty());

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetInternalState(BuffSetState {
                ex_info: None,
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(806, "ExPointOverflowBank"),
                },
                target_uid: 10,
                buff_uid: 2,
                params: Some("806#4".to_owned()),
                act_info: None,
            }),
        )
        .unwrap();
    assert_eq!(
        manager
            .snapshot(10, 2)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("806#4")
    );
    assert!(!changes.is_wire_visible());
    assert!(changes.events().is_empty());

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1026, "CreateMaxHpAdditionalDamageAndRemove"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::ExactId(20),
            }),
        )
        .unwrap();
    assert_eq!(
        changes
            .change
            .removed
            .iter()
            .filter_map(|removed| removed.buff.uid)
            .collect::<Vec<_>>(),
        vec![2, 4]
    );
    assert!(
        changes
            .change
            .removed
            .iter()
            .all(|removed| removed.config_effect == 1026)
    );
    assert!(!manager.has_buff_id(10, 20));
}

#[test]
fn dispel_plans_matching_statuses_and_preserves_behavior_provenance() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();

    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(530000111),
                        uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(530000112),
                        uid: Some(2),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Dispel(BuffDispel {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(30003, "Disperse1"),
                },
                target_uid: -1,
                statuses: vec![super::super::BuffStatus::PositiveStatus],
                excluded_ids_or_types: Vec::new(),
                count: 0,
            }),
        )
        .unwrap();
    assert_eq!(manager.active_for(-1).count(), 2);

    let changes = manager.commit(&HpManager::default(), plan);
    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(1));
    assert_eq!(changes.change.removed[0].config_effect, 30003);
    assert!(manager.has_buff_id(-1, 530000112));
}

#[test]
fn dispel_exclusions_preserve_matching_ids_or_types() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(4150001),
                        uid: Some(1),
                        count: Some(1),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(303),
                        uid: Some(2),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Dispel(BuffDispel {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60060, "DisperseExclude"),
                },
                target_uid: 10,
                statuses: vec![super::super::BuffStatus::NegativeStatus],
                excluded_ids_or_types: vec![4150001],
                count: 0,
            }),
        )
        .unwrap();
    manager.commit(&HpManager::default(), plan);

    assert!(manager.has_buff_id(10, 4150001));
    assert!(!manager.has_buff_id(10, 303));
}
