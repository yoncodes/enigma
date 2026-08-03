use sonettobuf::{
    BuffActInfo, BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo,
};

use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant, CommandOrigin},
        conduit::ConduitCommand,
        ex_point::{
            ExPointChange, ExPointChanges, ExPointCommand, ExPointConfigureSynchronization,
            ExPointRecordSynchronizationAction, SynchronizationDefinition,
        },
        hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

use super::{BattleManagers, base_hero_sp_attribute};

#[test]
fn hero_sp_attribute_wire_fields_follow_the_fight_version() {
    let version6 = base_hero_sp_attribute(6);
    let version7 = base_hero_sp_attribute(7);

    assert_eq!(version6.toughness_add, None);
    assert_eq!(version6.play_drop_rate2, None);
    assert_eq!(version7.toughness_add, Some(0));
    assert_eq!(version7.play_drop_rate2, Some(0));
}

#[test]
fn hero_sp_attributes_only_include_living_defenders() {
    crate::test_support::init_config();
    let entity = |uid, current_hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(1),
        team_type: Some(2),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 0), entity(-2, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        managers
            .hero_sp_attributes(&fight)
            .into_iter()
            .filter_map(|attribute| attribute.uid)
            .collect::<Vec<_>>(),
        vec![-2]
    );
}

#[test]
fn hero_upgrade_applies_configured_buffs_as_one_child_uid_sequence() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1094),
                    buff_id: Some(201),
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
        key: DefinitionKey::new(60037, "NotifyUpgradeHero"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut fight = fight;
    managers
        .execute_upgrade(super::upgrade::UpgradeCommand {
            owner_uid: 10,
            operation: super::upgrade::UpgradeOperation::Offer {
                origin,
                upgrade_id: 308665,
            },
        })
        .unwrap();

    let applied = managers
        .select_upgrade(&mut fight, 10, 308665, 3086515)
        .unwrap();
    let added = applied
        .buff_changes
        .iter()
        .filter_map(|changes| changes.change.added.as_ref())
        .map(|change| {
            (
                change.buff.buff_id.unwrap_or_default(),
                change.buff.uid.unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        added,
        vec![
            (30860132, 1095),
            (30860191, 1096),
            (30860171, 1097),
            (30860113, 1098),
        ]
    );
    assert!(
        applied
            .buff_changes
            .iter()
            .all(|changes| changes.origin == origin)
    );
    let entity = fight.attacker.as_ref().unwrap().entitys.first().unwrap();
    assert_eq!(
        managers.entity.passive_skills(10),
        Some(entity.passive_skill.as_slice())
    );
    let following = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31280112,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(
        following.change.added.unwrap().buff.uid,
        Some(1101),
        "the feature-backed layered state consumes its post-apply child UID"
    );
}

#[test]
fn successive_hero_upgrades_replace_the_skill_group_used_for_card_composition() {
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                skill_group1: vec![100, 101, 102],
                skill_group2: vec![110, 111, 112],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60037, "NotifyUpgradeHero"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = super::card::CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        },
    ]);
    managers.card.seed(&fight);
    let mut fight = fight;
    managers
        .apply_upgrade_identity(
            10,
            origin,
            &super::upgrade::UpgradeSelection {
                upgrade_id: 1,
                option_id: 2,
                add_buff_ids: Vec::new(),
                del_buff_ids: Vec::new(),
                replace_skill_group1: vec![200, 201, 202],
                replace_skill_group2: Vec::new(),
                replace_big_skill: 0,
                replace_passive_skills: Vec::new(),
                add_passive_skill_ids: Vec::new(),
            },
        )
        .unwrap();
    managers
        .apply_upgrade_identity(
            10,
            origin,
            &super::upgrade::UpgradeSelection {
                upgrade_id: 3,
                option_id: 4,
                add_buff_ids: Vec::new(),
                del_buff_ids: Vec::new(),
                replace_skill_group1: vec![300, 301, 302],
                replace_skill_group2: Vec::new(),
                replace_big_skill: 0,
                replace_passive_skills: Vec::new(),
                add_passive_skill_ids: Vec::new(),
            },
        )
        .unwrap();
    managers.sync_entities(&mut fight);
    let composed = managers
        .execute_card(super::card::CardCommand::ComposeAdjacent { origin })
        .unwrap();

    let entity = fight.attacker.as_ref().unwrap().entitys.first().unwrap();
    assert_eq!(entity.skill_group1, vec![300, 301, 302]);
    assert_eq!(composed.after.len(), 1);
    assert_eq!(composed.after[0].skill_id, Some(301));
}

#[test]
fn damage_batch_consumes_team_shared_shield_at_the_configured_target_count_rate() {
    crate::test_support::init_config();
    let ally = |uid, buffs| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let mut fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                ally(
                    1,
                    vec![BuffInfo {
                        uid: Some(50),
                        buff_id: Some(31430144),
                        from_uid: Some(1),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1125),
                            param: vec![5_000],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                ),
                ally(2, Vec::new()),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(1, "Damage"),
    };
    let damage = |target_uid, amount| {
        HpCommand::Damage(HpDamage {
            origin,
            source_uid: -1,
            target_uid,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 2,
                display_amount: None,
            },
        })
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_hp_batch(vec![damage(1, 418), damage(2, 367)])
        .unwrap();

    assert_eq!(managers.hp.current(1), 1_000);
    assert_eq!(managers.hp.current(2), 1_000);
    assert_eq!(
        changes[0].team_shared_shield_absorbed.unwrap().consumed,
        349
    );
    assert_eq!(
        changes[1].team_shared_shield_absorbed.unwrap().consumed,
        306
    );
    assert_eq!(
        managers
            .buff
            .snapshot(1, 50)
            .unwrap()
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1125))
            .unwrap()
            .param,
        vec![4_345]
    );

    managers.sync_entities(&mut fight);
    let mut reseeded = BattleManagers::seeded(&fight);
    let resumed = reseeded.execute_hp(damage(1, 120)).unwrap();
    let absorption = resumed.team_shared_shield_absorbed.unwrap();
    assert_eq!(absorption.before, 4_345);
    assert_eq!(absorption.after, 4_225);
    assert_eq!(
        reseeded
            .buff
            .snapshot(1, 50)
            .unwrap()
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1125))
            .unwrap()
            .param,
        vec![4_225]
    );
}

#[test]
fn damage_cap_applies_to_every_damage_instance_above_the_configured_limit() {
    crate::test_support::init_config();
    assert_eq!(
        super::buff::BuffManager::configured_features(610091)[0].raw,
        "510#300"
    );
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(uid * 10),
            buff_id: Some(610091),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1), entity(2), entity(3), entity(4)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(1, "Damage"),
    };
    let damage = |target_uid, amount, damage_from| {
        HpCommand::Damage(HpDamage {
            origin,
            source_uid: -1,
            target_uid,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 1,
                damage_from,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 2,
                display_amount: None,
            },
        })
    };
    let mut managers = BattleManagers::seeded(&fight);
    let cap_feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.owner_uid == 1)
        .unwrap();
    assert_eq!(cap_feature.raw, "510#300");
    assert_eq!(
        crate::engine::skill::buff_act::damage_not_more_than::cap(&cap_feature, &managers.hp),
        Some(300)
    );

    let changes = managers
        .execute_hp_batch(vec![
            damage(1, 400, HurtDamageFromType::Skill),
            damage(2, 200, HurtDamageFromType::Skill),
            damage(3, 400, HurtDamageFromType::SkillEffect),
            damage(4, 400, HurtDamageFromType::Buff),
        ])
        .unwrap();

    assert_eq!(changes[0].damage.unwrap().amount, 300);
    assert_eq!(changes[1].damage.unwrap().amount, 200);
    assert_eq!(changes[2].damage.unwrap().amount, 300);
    assert_eq!(changes[3].damage.unwrap().amount, 300);
    assert_eq!(
        (
            managers.hp.current(1),
            managers.hp.current(2),
            managers.hp.current(3),
            managers.hp.current(4)
        ),
        (700, 800, 700, 700)
    );
}

#[test]
fn depleted_team_shared_shield_removes_its_exact_carrier_before_reseed() {
    crate::test_support::init_config();
    let mut fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(50),
                    buff_id: Some(31430144),
                    from_uid: Some(1),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1125),
                        param: vec![100],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let damage = |target_uid| {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: -1,
            target_uid,
            amount: 120,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 2,
                display_amount: None,
            },
        })
    };

    assert_eq!(
        managers.execute_hp_batch(vec![damage(1), damage(999)]),
        Err(crate::engine::manager::hp::HpCommandError::MissingTarget(
            999
        ))
    );
    assert_eq!(managers.hp.current(1), 1_000);
    assert!(managers.buff.has_buff_id(1, 31430144));
    assert_eq!(
        managers
            .buff
            .snapshot(1, 50)
            .unwrap()
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1125))
            .unwrap()
            .param,
        vec![100]
    );

    let changes = managers.execute_hp(damage(1)).unwrap();

    let absorption = changes.team_shared_shield_absorbed.unwrap();
    assert_eq!((absorption.before, absorption.after), (100, 0));
    let removed = changes.team_shared_shield_removed.as_ref().unwrap();
    assert_eq!(removed.change.removed[0].buff.uid, Some(50));
    assert!(changes.events().iter().any(|event| {
        matches!(
            event,
            crate::engine::event::payload::BattleEvent::BuffRemoved(removed)
                if removed.buff_uid == 50
        )
    }));
    assert!(!managers.buff.has_buff_id(1, 31430144));

    managers.sync_entities(&mut fight);
    let reseeded = BattleManagers::seeded(&fight);
    assert!(!reseeded.buff.has_buff_id(1, 31430144));
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
    assert_eq!(attr.defense, Some(800));
    assert_eq!(attr.mdefense, Some(589));
    assert_eq!(base_attr.defense, Some(1_000));
    assert_eq!(base_attr.mdefense, Some(736));
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
    managers
        .entity
        .replace_team_roster(2, &[entity(-3)], &[entity(-2)]);
    managers.sync_entities(&mut fight);

    let uids = managers
        .ex_point_info(&fight)
        .into_iter()
        .filter_map(|info| info.uid)
        .collect::<Vec<_>>();
    assert_eq!(uids, vec![-2, -3]);
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
    let key = crate::engine::skill::rule::DefinitionKey::new(629210, "TeammateInjuryCount");

    assert!(managers.can_fire_rule(1, 433011, 0, key, 1, 0));
    managers.mark_rule_fired(1, 433011, 0, key);
    assert!(!managers.can_fire_rule(1, 433011, 0, key, 1, 0));
    managers.begin_round();
    assert!(!managers.can_fire_rule(1, 433011, 0, key, 1, 0));

    let round_key = crate::engine::skill::rule::DefinitionKey::new(629210, "DifferentCondition");
    assert!(managers.can_fire_rule(1, 433011, 1, round_key, 0, 1));
    managers.mark_rule_fired(1, 433011, 1, round_key);
    assert!(!managers.can_fire_rule(1, 433011, 1, round_key, 0, 1));
    managers.begin_round();
    assert!(managers.can_fire_rule(1, 433011, 1, round_key, 0, 1));
    assert!(managers.can_fire_rule(1, 433011, 2, round_key, 0, 0));
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
