use super::*;

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

#[test]
fn independent_instances_use_consecutive_normal_uids() {
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
        BuffCommand::GrantIndependent(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(884, "AddToBuffEntity3"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31_080_145,
            amount: Some(1),
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
    let second = manager
        .execute(&HpManager::default(), command())
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid;

    assert_eq!(first, Some(1002));
    assert_eq!(second, Some(1004));
}

#[test]
fn include_type_sixteen_refreshes_count_on_the_existing_uid() {
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
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(928, "AddToTarget"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31130123,
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
        .buff;
    let second = manager
        .execute(&HpManager::default(), command())
        .unwrap()
        .change
        .refreshed
        .pop()
        .unwrap()
        .after;

    assert_eq!(first.uid, Some(1002));
    assert_eq!(first.count, Some(1));
    assert_eq!(second.uid, first.uid);
    assert_eq!(second.count, Some(2));
}

#[test]
fn status_immunity_rejects_the_grant_and_consumes_its_charge() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(31270408),
                    uid: Some(20),
                    count: Some(1),
                    duration: Some(1),
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
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -2,
                target_uid: -1,
                buff_id: 4010,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(
        changes
            .change
            .rejected
            .as_ref()
            .map(|result| result.blocker_buff_id),
        Some(31270408)
    );
    assert_eq!(changes.change.removed.len(), 1);
    assert!(!manager.has_buff_id(-1, 31270408));
    assert!(!manager.has_buff_id(-1, 4010));
}

#[test]
fn static_control_immunity_rejects_without_consuming_the_carrier() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(5140006),
                    uid: Some(20),
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
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -2,
                target_uid: -1,
                buff_id: 4010,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(
        changes
            .change
            .rejected
            .as_ref()
            .map(|result| result.blocker_buff_id),
        Some(5140006)
    );
    assert!(changes.change.removed.is_empty());
    assert!(manager.has_buff_id(-1, 5140006));
    assert!(!manager.has_buff_id(-1, 4010));
}

#[test]
fn team_status_immunity_consumes_the_shared_carrier_budget() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    buffs: vec![BuffInfo {
                        buff_id: Some(31430144),
                        uid: Some(20),
                        from_uid: Some(10),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1126),
                            param: vec![2],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let grant_control = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: -1,
            target_uid: 11,
            buff_id: 4010,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    for remaining in [1, 0] {
        let changes = manager
            .execute(&HpManager::default(), grant_control())
            .unwrap();
        assert_eq!(
            changes
                .change
                .rejected
                .as_ref()
                .map(|result| result.blocker_buff_id),
            Some(31430144)
        );
        assert!(!manager.has_buff_id(11, 4010));
        assert_eq!(
            manager.snapshot(10, 20).unwrap().act_info[0].param,
            [remaining]
        );
    }

    let changes = manager
        .execute(&HpManager::default(), grant_control())
        .unwrap();
    assert!(changes.change.rejected.is_none());
    assert!(manager.has_buff_id(10, 31430144));
    assert!(manager.has_buff_id(11, 4010));
}

#[test]
fn rejected_layer_buff_uses_its_configured_child_uid_lane() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(530000417),
                    uid: Some(100055),
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
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 530000111,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(changes.change.rejected.unwrap().buff.uid, Some(100056));
}

#[test]
fn child_uid_grant_refreshes_an_existing_single_buff() {
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
    let command = || {
        BuffCommand::GrantUsingChildUid(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60142, "ConsumePowerAddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31130123,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let first = manager.execute(&hp, command()).unwrap();
    let first_uid = first.change.added.unwrap().buff.uid;
    let second = manager.execute(&hp, command()).unwrap();

    assert!(second.change.added.is_none());
    assert!(second.change.removed.is_empty());
    assert_eq!(second.change.refreshed.len(), 1);
    assert_eq!(second.change.refreshed[0].after.uid, first_uid);
}

#[test]
fn grant_snapshots_attribute_from_current_injury_bank() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30800141),
                        uid: Some(1),
                        from_uid: Some(10),
                        act_common_params: Some("770#174#3489".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(10_000),
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

    let changes = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60039, "RealDamageSelfAndAddBuffToTarget"),
                },
                source_uid: 10,
                target_uid: 20,
                buff_id: 30800111,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(
        changes
            .change
            .added
            .unwrap()
            .buff
            .act_common_params
            .as_deref(),
        Some("767#317")
    );
}

#[test]
fn enhanced_passive_variant_retains_its_state_and_silently_consumes_the_attempt_uid() {
    crate::test_support::init_config();
    let fight = Fight {
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
    };
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    manager.seed(&fight);
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

    let enhanced = manager.execute(&hp, grant(30030208)).unwrap();
    let base_attempt = manager.execute(&hp, grant(30030207)).unwrap();
    let following = manager.execute(&hp, grant(370002300)).unwrap();

    assert_eq!(enhanced.change.added.unwrap().buff.uid, Some(1002));
    assert_eq!(base_attempt.change, BuffReplaceResult::default());
    assert!(base_attempt.events().is_empty());
    assert!(manager.has_buff_id(10, 30030208));
    assert!(!manager.has_buff_id(10, 30030207));
    assert_eq!(following.change.added.unwrap().buff.uid, Some(1006));
}
