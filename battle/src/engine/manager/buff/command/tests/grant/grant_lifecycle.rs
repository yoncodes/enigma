use super::*;

#[test]
fn external_refresh_keeps_the_current_applier() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    buff_id: Some(103374114),
                    uid: Some(1109),
                    from_uid: Some(10),
                    duration: Some(2),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let changes = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -2,
                target_uid: 10,
                buff_id: 103374114,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::BuffChanged(change)] if change.source_uid == -2
    ));
}

#[test]
fn provoke_replaces_only_the_copy_from_the_same_applier() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(229103),
                        uid: Some(1105),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(229103),
                        uid: Some(1106),
                        from_uid: Some(20),
                        duration: Some(2),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let command = BuffCommand::Grant(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: 10,
        target_uid: -1,
        buff_id: 229103,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    });
    let plan = manager.plan(&hp, command.clone()).unwrap();
    assert_eq!(grant_plan(&plan).post_add_uids.len(), 1);

    let changes = manager.execute(&hp, command).unwrap();

    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(1105));
    assert!(manager.snapshot(-1, 1106).is_some());
    assert_eq!(
        manager
            .active_for(-1)
            .filter(|buff| buff.buff_id == Some(229103))
            .count(),
        2
    );
}

#[test]
fn replacement_removal_clears_the_stale_duration() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31450131),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let changes = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: -1,
                buff_id: 31450131,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(20));
    assert_eq!(changes.change.removed[0].buff.duration, Some(0));
    assert_eq!(
        changes.change.added.as_ref().unwrap().buff.duration,
        Some(3)
    );
}

#[test]
fn counted_channel_starts_from_its_configured_private_count() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31000441,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(added.buff.ex_info, Some(7));

    let unsupported = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 900045412,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(unsupported.buff.ex_info, Some(0));
}

#[test]
fn configured_round_bonus_extends_matching_buff_at_grant_time() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(72006),
                    uid: Some(1),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 30630112,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(added.buff.duration, Some(3));
}

#[test]
fn configured_type_round_bonus_extends_matching_buff_at_grant_time() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(21241),
                        uid: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 20,
                buff_id: 300704,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(added.buff.duration, Some(3));
}

#[test]
fn configured_type_round_bonus_scales_with_source_buff_count() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(11410011),
                        uid: Some(1),
                        from_uid: Some(10),
                        layer: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 20,
                buff_id: 300704,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(added.buff.duration, Some(4));
}

#[test]
fn stateful_grant_markers_use_the_committed_buff_params() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::GrantStateful(BuffGrantChild {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31080143,
                amount: None,
                params: Some("882#10".to_owned()),
                act_info: None,
            }),
        )
        .unwrap();

    let marker = changes
        .change
        .added
        .unwrap()
        .markers
        .into_iter()
        .find(|marker| {
            marker.effect_type == sonettobuf::effect_type_enum::EffectType::Fixattrteamenergy as i32
        })
        .unwrap();
    assert_eq!(marker.effect_num, 10);
}

#[test]
fn timed_fixed_hurt_reserves_the_next_normal_uid_after_its_first_add() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |buff_id| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let fixed = manager.execute(&hp, grant(2_112_021)).unwrap();
    let following = manager.execute(&hp, grant(5_230_012)).unwrap();

    assert_eq!(fixed.change.added.unwrap().buff.uid, Some(1002));
    assert_eq!(following.change.added.unwrap().buff.uid, Some(1006));

    let mut version_six = BuffManager::default();
    version_six.seed(&Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    version_six.execute(&hp, grant(2_112_021)).unwrap();
    let following = version_six.execute(&hp, grant(5_230_012)).unwrap();
    assert_eq!(following.change.added.unwrap().buff.uid, Some(4));
}

#[test]
fn permanent_fixed_hurt_does_not_reserve_a_normal_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    manager.seed(&Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let grant = |buff_id| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let fixed = manager.execute(&hp, grant(7_254_036)).unwrap();
    let following = manager.execute(&hp, grant(5_230_012)).unwrap();

    assert_eq!(fixed.change.added.unwrap().buff.uid, Some(1002));
    assert_eq!(following.change.added.unwrap().buff.uid, Some(1004));
}

#[test]
fn effect_count_consumption_is_internal_state_not_a_stack_change() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(530000711),
                    uid: Some(20),
                    count: Some(1),
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
            BuffCommand::ConsumeEffectCount(BuffConsume {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(503, "AddToTarget"),
                },
                target_uid: -1,
                selector: BuffSelector::Uid(20),
                amount: 1,
                depleted: DepletedBuff::Keep,
            }),
        )
        .unwrap();

    assert_eq!(manager.snapshot(-1, 20).unwrap().count, Some(0));
    assert!(manager.has_buff_id(-1, 530000711));
    assert_eq!(changes.change, BuffReplaceResult::default());
    assert!(changes.events().is_empty());
}

#[test]
fn action_expired_add_to_target_carrier_reserves_child_before_reapply() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: -1,
            target_uid: -1,
            buff_id: 530000712,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let first = manager
        .execute(&HpManager::default(), grant())
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    let expired = manager
        .execute(
            &HpManager::default(),
            BuffCommand::ExpireAction(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(9, "OwnerCastSkillSlot"),
                },
                target_uid: -1,
                selector: BuffRemoveSelector::Uid(first),
            }),
        )
        .unwrap();
    assert_eq!(expired.change.removed[0].buff.count, Some(0));
    assert_eq!(expired.change.removed[0].config_effect, 0);

    let plan = manager
        .plan(&HpManager::default(), grant())
        .expect("reapply plans");
    let plan = grant_plan(&plan);
    assert_eq!(plan.pre_add_uids.len(), 1);
    assert_eq!(plan.uid.unwrap().uid, first + 3);
}

#[test]
fn action_expiry_preserves_the_removed_stack_layer() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2130015),
                    from_uid: Some(-1),
                    layer: Some(2),
                    count: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let expired = manager
        .execute(
            &HpManager::default(),
            BuffCommand::ExpireAction(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(9, "OwnerCastSkillSlot"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(20),
            }),
        )
        .unwrap();

    assert_eq!(expired.change.removed[0].buff.layer, Some(2));
    assert_eq!(expired.change.removed[0].buff.count, Some(0));
}

#[test]
fn emitter_attack_count_carrier_reserves_two_normal_uids_before_reapply() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let command = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31080111,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let first = manager
        .execute(&HpManager::default(), command())
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid;
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(3, "RoundEnd"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(first.unwrap()),
            }),
        )
        .unwrap();
    let planned = manager.plan(&HpManager::default(), command()).unwrap();
    let grant = grant_plan(&planned);

    assert_eq!(first, Some(1002));
    assert_eq!(
        grant
            .pre_add_uids
            .iter()
            .map(|plan| plan.uid)
            .collect::<Vec<_>>(),
        vec![1004, 1006]
    );
    assert_eq!(grant.uid.unwrap().uid, 1008);
}

#[test]
fn unrelated_bare_include_type_seven_does_not_reserve_emitter_uids() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let command = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 30480241,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let first = manager
        .execute(&HpManager::default(), command())
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(3, "RoundEnd"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(first),
            }),
        )
        .unwrap();
    let planned = manager.plan(&HpManager::default(), command()).unwrap();
    let grant = grant_plan(&planned);

    assert_eq!(first, 1002);
    assert!(grant.pre_add_uids.is_empty());
    assert_eq!(grant.uid.unwrap().uid, 1004);
}
