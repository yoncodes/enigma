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
