use super::*;

#[test]
fn lock_hp_caps_damage_and_hp_loss_at_the_configured_max_hp_floor() {
    crate::test_support::init_config();
    let target = |uid, buff_uid| {
        let mut target =
            crate::engine::fight::defender::Defender::build_monster_with_uid(119383111, uid, 1, 2)
                .unwrap();
        target.current_hp = Some(27_082);
        target.attr.as_mut().unwrap().hp = Some(27_082);
        target.base_attr.as_mut().unwrap().hp = Some(27_082);
        target.buffs.push(BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(119381136),
            from_uid: Some(uid),
            count: Some(1),
            layer: Some(1),
            duration: Some(1),
            ..Default::default()
        });
        target
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![target(-1, 1), target(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_hp(HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(-1, "ConfiguredDamage"),
            },
            source_uid: 10,
            target_uid: -1,
            amount: 52_056,
            config_effect: -1,
            effect_kind: DamageEffectKind::Critical,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: true,
                career_restraint: true,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();

    assert_eq!(changes.damage.unwrap().amount, 26_812);
    assert_eq!(changes.applied_damage(), 26_812);
    assert_eq!(managers.hp.current(-1), 270);
    assert!(!changes.caused_death());

    let changes = managers
        .execute_hp(HpCommand::Lose(HpLoss {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(-2, "ConfiguredHpLoss"),
            },
            source_uid: 10,
            target_uid: -2,
            amount: 52_056,
            config_effect: -1,
            hurt: None,
        }))
        .unwrap();

    assert_eq!(changes.applied_damage(), 26_812);
    assert_eq!(managers.hp.current(-2), 270);
    assert!(!changes.caused_death());
}

#[test]
fn summoned_monster_snapshot_includes_its_configured_moxie_maximum() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 116_330_100,
                position: 2,
            },
        })
        .unwrap();

    assert_eq!(changes.entity.ex_point_max, Some(0));
}

#[test]
fn entity_sync_projects_current_primary_attributes_without_rewriting_the_base() {
    crate::test_support::init_config();
    let base = HeroAttribute {
        attack: Some(1_911),
        defense: Some(763),
        mdefense: Some(752),
        technic: Some(1_106),
        hp: Some(10_609),
        ..Default::default()
    };
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                attr: Some(base),
                base_attr: Some(base),
                current_hp: Some(10_609),
                buffs: vec![BuffInfo {
                    buff_id: Some(312401461),
                    uid: Some(1_001),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    managers.sync_entities(&mut fight);

    let entity = &fight.attacker.as_ref().unwrap().entitys[0];
    assert_eq!(entity.attr.as_ref().unwrap().attack, Some(2_044));
    assert_eq!(entity.base_attr.as_ref().unwrap().attack, Some(1_911));
}

#[test]
fn transform_projects_active_primary_attribute_buffs_without_rewriting_the_base() {
    crate::test_support::init_config();
    let base = HeroAttribute {
        hp: Some(67_680),
        attack: Some(1_696),
        defense: Some(1_000),
        mdefense: Some(736),
        technic: Some(210),
        ..Default::default()
    };
    let mut original =
        crate::engine::fight::defender::Defender::build_monster_with_uid(30111001, -1, 1, 2)
            .unwrap();
    original.current_hp = base.hp;
    original.attr = Some(base);
    original.base_attr = Some(base);
    original.buffs = vec![
        BuffInfo {
            buff_id: Some(300403),
            uid: Some(1_001),
            from_uid: Some(10),
            ..Default::default()
        },
        BuffInfo {
            buff_id: Some(300404),
            uid: Some(1_002),
            from_uid: Some(10),
            ..Default::default()
        },
    ];
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![original],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 30111005,
                parameters: [1000, 0],
            },
        })
        .unwrap();

    let attr = changes.entity.attr.unwrap();
    let base_attr = changes.entity.base_attr.unwrap();
    assert_eq!(changes.entity.ex_point_max, Some(0));
    assert_eq!(attr.defense, Some(800));
    assert_eq!(attr.mdefense, Some(589));
    assert_eq!(base_attr.defense, Some(1_000));
    assert_eq!(base_attr.mdefense, Some(736));
}

#[test]
fn transform_reset_mode_removes_carried_buffs_from_manager_state() {
    crate::test_support::init_config();
    let mut original =
        crate::engine::fight::defender::Defender::build_monster_with_uid(251417, -7, 1, 2).unwrap();
    original.buffs.push(BuffInfo {
        buff_id: Some(229103),
        uid: Some(100_007),
        from_uid: Some(-7),
        ..Default::default()
    });
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![original],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -7,
            target_uid: -7,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 251407,
                parameters: [1000, 1],
            },
        })
        .unwrap();

    assert!(changes.entity.buffs.is_empty());
    assert!(!managers.buff.has_buff_id(-7, 229103));
}

#[test]
fn entity_projection_includes_passive_skills_linked_by_active_buffs() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![42],
                buffs: vec![BuffInfo {
                    buff_id: Some(30650202),
                    uid: Some(1002),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    managers.sync_entities(&mut fight);

    assert_eq!(
        fight.attacker.unwrap().entitys[0].passive_skill,
        vec![42, 30650201]
    );
}

#[test]
fn ex_point_info_uses_synced_manager_state() {
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(0),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ex_point_type: Some(1),
                power_infos: vec![PowerInfo {
                    power_id: Some(2),
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
    managers.ex_point.add(10, 10, 4, 0);
    managers.hp.lose(10, 25, 0);
    managers.eureka.set(10, 2, 3);
    managers.sync_entities(&mut fight);

    let info = managers.ex_point_info(&fight);
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].uid, Some(10));
    assert_eq!(info[0].ex_point, Some(4));
    assert_eq!(info[0].current_hp, Some(75));
    assert_eq!(info[0].ex_point_type, Some(1));
    assert_eq!(info[0].power_infos[0].num, Some(3));
}

#[test]
fn ex_point_info_uses_current_roster_in_encounter_order() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2)],
            sub_entitys: vec![entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = managers.catalog();
    managers
        .entity
        .replace_team_roster(catalog, 2, &[entity(-3)], &[entity(-2)]);
    managers.sync_entities(&mut fight);

    let uids = managers
        .ex_point_info(&fight)
        .into_iter()
        .filter_map(|info| info.uid)
        .collect::<Vec<_>>();
    assert_eq!(uids, vec![-2, -3]);
}

#[test]
fn ex_point_info_keeps_a_defeated_combatant_replaced_by_a_summon() {
    crate::test_support::init_config();
    let entity = |uid, position, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1, 100), entity(-2, 2, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let summoned = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 116_330_100,
                position: 2,
            },
        })
        .unwrap()
        .entity
        .uid
        .unwrap();
    managers.sync_entities(&mut fight);

    assert_eq!(
        managers
            .ex_point_info(&fight)
            .into_iter()
            .filter_map(|info| info.uid)
            .collect::<Vec<_>>(),
        vec![-1, summoned, -2]
    );
}

#[test]
fn buff_commands_do_not_mutate_the_separately_owned_eureka_cap() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                power_infos: vec![PowerInfo {
                    power_id: Some(1),
                    num: Some(0),
                    max: Some(6),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "PowerMaxAdd"),
    };
    let mut managers = BattleManagers::seeded(&fight);

    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31050147,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    assert_eq!(managers.eureka.get(10, 1).max, 6);
}

#[test]
fn synchronization_reconnect_state_is_projected_from_ex_point_manager() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point_type: Some(2),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(229100),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100000, "EzioProps"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_ex_point(ExPointCommand::ConfigureSynchronization(
            ExPointConfigureSynchronization {
                origin,
                target_uid: 10,
                definition: SynchronizationDefinition::new([1, 2, 3], 4, 100).unwrap(),
            },
        ))
        .unwrap();
    managers
        .execute_ex_point(ExPointCommand::RecordSynchronizationAction(
            ExPointRecordSynchronizationAction {
                origin,
                target_uid: 10,
                action_target_uid: -1,
                damage: 123,
            },
        ))
        .unwrap();

    managers.sync_entities(&mut fight);

    let buff = &fight.attacker.as_ref().unwrap().entitys[0].buffs[0];
    assert_eq!(buff.act_common_params.as_deref(), Some("10000#2,123,-1"));
    assert!(
        buff.act_info
            .iter()
            .any(|info| { info.act_id == Some(10000) && info.param == [2, 123, -1] })
    );
}

#[test]
fn total_and_round_rule_limits_have_distinct_lifetimes() {
    let mut managers = BattleManagers::default();
    let owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(0);

    assert!(managers.can_fire_rule(1, 433011, owner, 1, 0));
    managers.mark_rule_fired(1, 433011, owner);
    assert!(!managers.can_fire_rule(1, 433011, owner, 1, 0));
    managers.begin_round();
    assert!(!managers.can_fire_rule(1, 433011, owner, 1, 0));

    let round_owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(1);
    assert!(managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    managers.mark_rule_fired(1, 433011, round_owner);
    assert!(!managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    managers.begin_round();
    assert!(managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    assert!(managers.can_fire_rule(
        1,
        433011,
        crate::engine::skill::effect::RuleLimitOwner::from_slot(2),
        0,
        0
    ));
}

#[test]
fn action_round_state_preserves_next_round_reaction_usage() {
    let mut managers = BattleManagers::default();
    let owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(0);

    managers.mark_rule_fired(-1, 100, owner);
    managers.begin_reaction_round();
    assert!(managers.can_fire_rule(-1, 100, owner, 0, 1));

    managers.mark_rule_fired(-1, 100, owner);
    managers.begin_action_round();
    assert!(!managers.can_fire_rule(-1, 100, owner, 0, 1));
}

#[test]
fn rule_progress_ignores_non_positive_movement() {
    let mut managers = BattleManagers::default();
    let key = DefinitionKey::new(1, "Progress");

    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, -1), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 5), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 1), 1);
}

#[test]
fn rule_progress_is_owned_by_the_exact_listener_instance() {
    let mut managers = BattleManagers::default();
    let key = DefinitionKey::new(1, "Progress");

    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 5), 0);
    assert_eq!(managers.advance_rule_progress(10, 21, key, 6, 1), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 1), 1);
}

#[test]
fn ex_point_types_use_their_own_default_caps() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_with_ex_point_type(1, 0),
                entity_with_ex_point_type(2, 1),
                entity_with_ex_point_type(3, 2),
                entity_with_ex_point_type(4, 3),
                entity_with_ex_point_type(5, 999),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.ex_point.add(1, 1, 10, 0).after, 5);
    assert_eq!(managers.ex_point.add(2, 2, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(3, 3, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(4, 4, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(5, 5, 10, 0).after, 10);
}

#[test]
fn ex_point_cant_add_blocks_gains_but_allows_spending() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(3),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(603, "ExPointCantAdd"),
    };
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31050132,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let change = |delta| {
        ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: 10,
            target_uid: 10,
            delta,
            config_effect: 0,
            effect_type: 0,
        })
    };
    let blocked = managers.execute_ex_point(change(2)).unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.requested_delta == 2
                && change.applied_delta == 0
                && change.before == 3
                && change.after == 3
    ));
    let spent = managers.execute_ex_point(change(-2)).unwrap();
    assert!(matches!(
        spent,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == -2 && change.after == 1
    ));
}

#[test]
fn transfer_add_ex_point_redirects_only_gains_to_the_buff_source() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_point: Some(0),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    ex_point: Some(1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31_000_171),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(833, "TransferAddExPoint"),
    };

    let gained = managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: 20,
            target_uid: 20,
            delta: 2,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert!(matches!(
        gained,
        ExPointChanges::Value { change, .. }
            if change.source_uid == 20
                && change.target_uid == 10
                && change.applied_delta == 2
    ));
    assert_eq!(managers.ex_point.get(10), 2);
    assert_eq!(managers.ex_point.get(20), 1);

    managers
        .execute_ex_point(ExPointCommand::Spend(ExPointChange {
            origin,
            source_uid: 20,
            target_uid: 20,
            delta: -1,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert_eq!(managers.ex_point.get(10), 2);
    assert_eq!(managers.ex_point.get(20), 0);
}

#[test]
fn skill_damage_reduces_guard_at_the_active_action_rate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                toughness_value: Some(5_000),
                toughness_point: Some(3),
                is_broken: Some(false),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let damage = || {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            amount: 1_000,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: -1_000,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: Some(1_000),
            },
        })
    };

    let normal = managers.execute_hp(damage()).unwrap();
    assert_eq!(normal.toughness.unwrap().value_delta, 200);

    managers
        .conduit
        .execute(ConduitCommand::SetRunning {
            source_uid: 10,
            running: true,
        })
        .unwrap();
    let conduit = managers.execute_hp(damage()).unwrap();
    assert_eq!(conduit.toughness.unwrap().value_delta, 1_000);
}

#[test]
fn moxie_reduction_immunity_blocks_reduction_but_allows_spending() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(3),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let guard_origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(509, "ImmunityExpointChange"),
    };
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: guard_origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 5081,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let reduction = |delta| {
        ExPointCommand::Change(ExPointChange {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(605, "ExPointDel"),
            },
            source_uid: 10,
            target_uid: 10,
            delta,
            config_effect: 0,
            effect_type: 0,
        })
    };
    let blocked = managers.execute_ex_point(reduction(-2)).unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.requested_delta == -2
                && change.applied_delta == 0
                && change.before == 3
                && change.after == 3
    ));
    let spent = managers
        .execute_ex_point(ExPointCommand::Spend(ExPointChange {
            origin: crate::engine::manager::card::CARD_PLAY_ORIGIN,
            source_uid: 10,
            target_uid: 10,
            delta: -2,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert!(matches!(
        spent,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == -2 && change.after == 1
    ));
}

#[test]
fn moxie_reduction_immunity_precedes_resistance() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(103370111),
                ex_point: Some(3),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let granted = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 5081,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let buff_uid = granted.change.added.unwrap().buff.uid.unwrap();
    let reduction = || {
        ExPointCommand::Reduce(ExPointChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(30001, "DelExPoint"),
            },
            source_uid: 10,
            target_uid: -1,
            delta: -1,
            config_effect: 1,
            effect_type: 0,
        })
    };
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    determinism.replace_moxie_reduction_resistance_choices([
        crate::engine::runtime::determinism::MoxieReductionResistanceChoice {
            producer_id: 30001,
            source_uid: 10,
            target_uid: -1,
            resisted: true,
        },
    ]);

    let blocked = managers
        .execute_ex_point_rolled(reduction(), &mut determinism)
        .unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == 0 && change.before == 3 && change.after == 3
    ));

    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            target_uid: -1,
            selector: BuffRemoveSelector::Uid(buff_uid),
        }))
        .unwrap();
    assert!(matches!(
        managers
            .execute_ex_point_rolled(reduction(), &mut determinism)
            .unwrap(),
        ExPointChanges::Resisted { target_uid: -1, .. }
    ));
}

fn entity_with_ex_point_type(uid: i64, ex_point_type: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        ex_point: Some(0),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ex_point_type: Some(ex_point_type),
        ..Default::default()
    }
}
