use super::*;

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
fn hero_sp_attributes_follow_the_fight_version_not_record_mode() {
    crate::test_support::init_config();
    let mut fight = Fight {
        version: Some(7),
        is_record: Some(true),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        managers.hero_sp_attributes(&fight)[0]
            .attribute
            .as_ref()
            .and_then(|attribute| attribute.device_skill_rate),
        Some(0)
    );
    fight.is_record = Some(false);
    assert_eq!(
        managers.hero_sp_attributes(&fight)[0]
            .attribute
            .as_ref()
            .and_then(|attribute| attribute.device_skill_rate),
        Some(0)
    );
}

#[test]
fn fight_snapshot_projects_team_lingering_glow_and_crystals() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31340008),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let key = crate::engine::mechanic::lingering_glow::key(1);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestLingeringGlow"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::ChangeValue { delta: 121 },
        ))
        .unwrap();
    crate::engine::mechanic::heat_scale::HeatScale::select_crystals_from_cloth(
        &mut managers,
        10,
        110,
    )
    .unwrap();

    managers.sync_entities(&mut fight);

    assert_eq!(
        fight.attacker.unwrap().heat_scale,
        Some(sonettobuf::FightHeatScale {
            value: Some(121),
            max: Some(750),
            crystal: Some(110),
        })
    );
}

#[test]
fn hero_sp_attributes_project_active_playmode_immunity_until_removed() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1),
                team_type: Some(2),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(116362001),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_sp(
        -1,
        &HeroSpAttribute {
            clutch: Some(50),
            normal_skill_rate: Some(50),
            ..Default::default()
        },
    );

    let projected_attribute = |managers: &BattleManagers| {
        managers
            .hero_sp_attributes(&fight)
            .into_iter()
            .find(|attribute| attribute.uid == Some(-1))
            .and_then(|attribute| attribute.attribute)
    };

    let attribute = projected_attribute(&managers).unwrap();
    assert_eq!(attribute.clutch, Some(50));
    assert_eq!(attribute.normal_skill_rate, Some(50));
    assert_eq!(attribute.play_drop_rate, Some(300));

    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "RemoveBuff"),
            },
            target_uid: -1,
            selector: BuffRemoveSelector::ExactId(116362001),
        }))
        .unwrap();

    assert_eq!(
        projected_attribute(&managers).and_then(|attribute| attribute.play_drop_rate),
        Some(0)
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
        .execute_upgrade(super::super::upgrade::UpgradeCommand {
            owner_uid: 10,
            operation: super::super::upgrade::UpgradeOperation::Offer {
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
        Some(1100),
        "the visible stacked state has no extra post-apply reservation"
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
    managers.card = super::super::card::CardManager::new(vec![
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
            &super::super::upgrade::UpgradeSelection {
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
            &super::super::upgrade::UpgradeSelection {
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
        .execute_card(super::super::card::CardCommand::ComposeAdjacent { origin })
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
        super::super::buff::BuffManager::configured_features(610091)[0].raw,
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
