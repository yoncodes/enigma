use super::*;

#[test]
fn normalizes_burn_buff_type() {
    crate::test_support::init_config();

    assert_eq!(
        BattleCatalog::new(crate::test_support::game_data()).burn_buff_type_id(),
        Some(4_150_001)
    );
}

#[test]
fn normalizes_target_counts() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.target_count(1), 1);
    assert_eq!(catalog.damage_target_count_kind(1), 1);
    assert_eq!(catalog.damage_target_count_kind(201), 2);
    assert_eq!(catalog.damage_target_count_kind(202), 2);
    assert_eq!(catalog.target_count(i32::MAX), 0);
    assert_eq!(catalog.damage_target_count_kind(i32::MAX), 0);
}

#[test]
fn normalizes_summoned_unique_skills() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.summoned_unique_skills(15_001), vec![30_740_171]);
    assert!(catalog.summoned_unique_skills(-1).is_empty());
}

#[test]
fn normalizes_hero_upgrade_options() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.upgrade_selection(308_664, 3_086_524),
        Some(crate::engine::manager::upgrade::UpgradeSelection {
            upgrade_id: 308_664,
            option_id: 3_086_524,
            add_buff_ids: vec![30_860_132, 30_860_191, 30_860_172, 30_860_112],
            del_buff_ids: vec![30_860_131],
            replace_skill_group1: Vec::new(),
            replace_skill_group2: vec![30_865_127, 30_865_128, 30_865_129],
            replace_big_skill: 0,
            replace_passive_skills: Vec::new(),
            add_passive_skill_ids: Vec::new(),
        })
    );
    assert_eq!(catalog.upgrade_selection(308_664, 3_086_525), None);
    assert_eq!(
        catalog.upgrade_has_available_option(308_664, &[]),
        Some(true)
    );
    assert_eq!(
        catalog.upgrade_has_available_option(308_664, &[3_086_514, 3_086_524, 3_086_534]),
        Some(false)
    );
    assert_eq!(catalog.upgrade_has_available_option(-1, &[]), None);
}

#[test]
fn normalizes_entity_ex_point_max() {
    crate::test_support::init_config();
    let game_data = crate::test_support::game_data();

    assert_eq!(
        configured_ex_point_max(game_data, None, Some(3120), 180),
        Some(8)
    );
    assert_eq!(
        configured_ex_point_max(game_data, Some(17), Some(3120), 180),
        Some(17)
    );
    assert_eq!(
        configured_ex_point_max(game_data, None, Some(109_360_002), 1),
        Some(2)
    );
    assert_eq!(
        configured_ex_point_max(game_data, None, Some(4_030_703), 1),
        Some(5)
    );
    assert_eq!(configured_ex_point_max(game_data, None, None, 1), None);
    assert_eq!(configured_ex_point_max(game_data, None, Some(-1), 1), None);
}

#[test]
fn normalizes_monster_toughness() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.monster_toughness(109_350_003, 1_015_000),
        Some((101_500, 4))
    );
    assert_eq!(catalog.monster_toughness(-1, 1_015_000), None);
}

#[test]
fn normalizes_lingering_glow_attribute_buff() {
    crate::test_support::init_config();

    assert_eq!(
        BattleCatalog::new(crate::test_support::game_data()).lingering_glow_attribute_buff(),
        Some(LingeringGlowAttributeBuff {
            buff_id: 31340007,
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: crate::engine::skill::rule::DefinitionKey::new(1053, "AttrByHeatScale"),
            },
        })
    );
}

#[test]
fn normalizes_fight_version() {
    crate::test_support::init_config();
    let game_data = crate::test_support::game_data();

    assert_eq!(
        BattleCatalog::new(game_data).fight_version(),
        ConfiguredFightVersion::Value(game_data.r#const.get(1707).unwrap().value.parse().unwrap())
    );
    assert_eq!(
        configured_fight_version(None),
        ConfiguredFightVersion::Missing
    );
    assert_eq!(
        configured_fight_version(Some("not-an-integer")),
        ConfiguredFightVersion::Invalid
    );
}

#[test]
fn normalizes_impromptu_definition() {
    crate::test_support::init_config();
    let game_data = crate::test_support::game_data();
    let catalog = BattleCatalog::new(game_data);
    let definition = catalog.impromptu_definition().unwrap();

    assert_eq!(
        definition.skill_id(),
        game_data
            .fight_asfd_const
            .get(5)
            .unwrap()
            .value
            .parse::<i32>()
            .unwrap()
    );
    assert_eq!(
        game_data
            .buff_act
            .get(definition.damage_up_act_id())
            .unwrap()
            .r#type,
        "EmitterDamageUp"
    );
    assert_eq!(
        definition.damage_rate(2),
        game_data
            .fight_asfd_const
            .get(6)
            .unwrap()
            .value
            .parse::<i32>()
            .unwrap()
            * 2
    );
}

#[test]
fn normalizes_magic_circle_attributes_and_thresholds() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.magic_circle(30001),
        Some(MagicCircleDefinition {
            duration: 3,
            allied_attributes: vec![(205, 150)],
            enemy_attributes: Vec::new(),
            allied_buffs: Vec::new(),
            enemy_buffs: Vec::new(),
            self_skills: Vec::new(),
        })
    );
    assert_eq!(
        catalog.magic_circle_thresholds(),
        vec![
            FieldThreshold {
                level: 1,
                progress: 0,
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
            },
            FieldThreshold {
                level: 2,
                progress: 50,
                definition: FieldDefinition {
                    field_id: 30002,
                    duration: 3,
                },
            },
            FieldThreshold {
                level: 3,
                progress: 120,
                definition: FieldDefinition {
                    field_id: 30003,
                    duration: 2,
                },
            },
        ]
    );
}

#[test]
fn normalizes_magic_circle_linked_battle_rules() {
    crate::test_support::init_config();

    let blood_domain = BattleCatalog::new(crate::test_support::game_data())
        .magic_circle(100051)
        .unwrap();

    assert_eq!(blood_domain.allied_buffs, vec![308801312]);
    assert_eq!(blood_domain.self_skills, vec![308801821]);
}

#[test]
fn normalizes_buff_consumption_and_action_expiry() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert!(catalog.buff_has_effect_count(6240530));
    assert!(!catalog.buff_has_effect_count(610091));
    assert!(catalog.buff_has_effect_count(90201));
    assert!(!catalog.buff_expires_after_owner_attack(90201));
    assert!(catalog.buff_expires_after_owner_attack(2220010));
    assert!(!catalog.buff_has_effect_count(-1));
    assert!(!catalog.buff_expires_after_owner_attack(-1));
}

#[test]
fn normalizes_configured_buff_features_in_order() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.buff_features(31260151),
        vec![
            ConfiguredBuffFeature {
                act_type: "CreateMaxHpAdditionalDamageAndRemove".to_owned(),
                effect_time: 203,
                effect_condition: 0,
                raw: "1026#1#750#31260171".to_owned(),
                values: vec![1026, 1, 750, 31260171],
            },
            ConfiguredBuffFeature {
                act_type: "SubBuff".to_owned(),
                effect_time: 0,
                effect_condition: 0,
                raw: "933#31260201".to_owned(),
                values: vec![933, 31260201],
            },
            ConfiguredBuffFeature {
                act_type: "Bullet".to_owned(),
                effect_time: 208,
                effect_condition: 3,
                raw: "827".to_owned(),
                values: vec![827],
            },
        ]
    );
    assert!(catalog.buff_features(-1).is_empty());
}

#[test]
fn normalizes_buff_feature_tokens_and_registry_identity() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert!(catalog.buff_has_master_halo(30860161));
    assert!(!catalog.buff_has_master_halo(-1));

    assert_eq!(
        catalog.buff_feature_tokens(109320111),
        vec!["704#1#0", "100#211#50", "100#214#50", "100#206#50",]
    );
    assert_eq!(
        catalog
            .buff_act_definition(704)
            .map(|definition| definition.key),
        Some(crate::engine::skill::rule::DefinitionKey::new(
            704, "HaloBase"
        ))
    );
    assert!(catalog.buff_feature_tokens(-1).is_empty());
    assert!(catalog.buff_act_definition(-1).is_none());
}

#[test]
fn resolves_exact_buff_act_command_origins() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.buff_act_origin(
            1062,
            crate::engine::skill::buff_act::registry::BuffActKind::HeatScaleDecrCounter,
        ),
        Some(CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: crate::engine::skill::rule::DefinitionKey::new(1062, "HeatScaleDecrCounter",),
        })
    );
    assert!(
        catalog
            .buff_act_origin(
                1062,
                crate::engine::skill::buff_act::registry::BuffActKind::AddAttrBySpecialCount,
            )
            .is_none()
    );
    assert!(
        catalog
            .buff_act_origin(
                -1,
                crate::engine::skill::buff_act::registry::BuffActKind::HeatScaleDecrCounter,
            )
            .is_none()
    );
}

#[test]
fn normalizes_random_buff_pools_in_order() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.buff_pool(30830151),
        Some(vec![308301511, 308301512, 308301513, 308301514])
    );
    assert!(catalog.buff_pool(-1).is_none());
}

#[test]
fn normalizes_card_skill_metadata() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.skill_effect_id(30020131), 710331);
    assert_eq!(catalog.skill_big_skill_point(30020131), 5);
    assert!(catalog.skill_is_big(30020131));
    assert_eq!(catalog.skill_effect_tag(30020131), 3);
    assert_eq!(catalog.skill_extra_kind(30020131), 0);
    assert_eq!(catalog.skill_type(30020131), 0);
    assert!(catalog.skill_is_attack(30020131));
    assert_eq!(catalog.skill_big_skill_point(30610131), 5);
    assert_eq!(catalog.skill_big_skill_point(31390111), 0);
    assert!(catalog.skill_is_big(30610131));
    assert!(!catalog.skill_is_big(31390111));
    assert_eq!(catalog.skill_effect_tag(31446011), 14);
    assert_eq!(catalog.skill_effect_tag(31390111), 3);
    assert!(catalog.skill_is_ultimate_for_model(31340131, 3134));
    assert!(!catalog.skill_is_ultimate_for_model(31340111, 3134));
    assert!(!catalog.skill_is_ultimate_for_model(31340131, 3139));
    assert!(!catalog.skill_is_ultimate_for_model(-1, 3134));
    assert_eq!(catalog.skill_big_skill_point(-1), 0);
    assert!(!catalog.skill_is_big(-1));
    assert_eq!(catalog.skill_effect_tag(-1), 0);
    assert_eq!(catalog.skill_extra_kind(-1), 0);
    assert_eq!(catalog.skill_type(-1), 0);
    assert!(!catalog.skill_is_attack(-1));
}

#[test]
fn normalizes_player_skills_in_slot_order() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.player_skills(Some(1)),
        vec![
            ConfiguredPlayerSkill {
                skill_id: 30010201,
                need_power: Some(40),
            },
            ConfiguredPlayerSkill {
                skill_id: 30010202,
                need_power: Some(25),
            },
        ]
    );
    assert_eq!(catalog.player_skills(None), catalog.player_skills(Some(1)));
    assert!(catalog.player_skills(Some(-1)).is_empty());
}

#[test]
fn normalizes_cloth_power_and_skill_terms() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            cloth_id: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        catalog.cloth_power(&fight),
        ClothPower::for_fight(crate::test_support::game_data(), &fight)
    );
    assert_eq!(
        catalog.cloth_skill_terms(Some(1), 30010201, 0),
        Some((40, 50, 1))
    );
    assert_eq!(
        catalog.cloth_skill_terms(Some(1), 30010201, 99),
        Some((60, 60, 1))
    );
    assert_eq!(
        catalog.cloth_skill_terms(Some(1), 30010202, 0),
        Some((25, 25, 0))
    );
    assert!(
        catalog
            .cloth_power(&sonettobuf::Fight {
                attacker: Some(sonettobuf::FightTeam {
                    cloth_id: Some(-1),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .is_none()
    );
    assert!(catalog.cloth_skill_terms(Some(1), -1, 0).is_none());
}

#[test]
fn normalizes_damage_affinity_data() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.fight_const_value(11), 100);
    assert_eq!(catalog.fight_const_value(12), 150);
    assert_eq!(catalog.fight_const_value(13), 300);
    assert_eq!(catalog.fight_const_value(14), 0);
    assert_eq!(catalog.fight_const_value(-1), 0);
    assert_eq!(catalog.career_multiplier(1, 4), 1300);
    assert_eq!(catalog.career_multiplier(1, 1), 1000);
    assert_eq!(catalog.career_multiplier(1, -1), 1000);
    assert_eq!(catalog.career_multiplier(-1, 4), 1000);
    assert_eq!(catalog.strongest_career_multiplier(1), 1300);
    assert_eq!(catalog.strongest_career_multiplier(-1), 1000);
}

#[test]
fn normalizes_current_wave_boss_models() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.boss_model_ids(&sonettobuf::Fight {
            episode_id: Some(90001601),
            ..Default::default()
        }),
        vec![900016101, 900016102]
    );
    assert!(
        catalog
            .boss_model_ids(&sonettobuf::Fight {
                episode_id: Some(90001601),
                battle_id: Some(i32::MAX),
                ..Default::default()
            })
            .is_empty()
    );
    assert!(
        catalog
            .boss_model_ids(&sonettobuf::Fight {
                episode_id: Some(90001601),
                cur_wave: Some(2),
                ..Default::default()
            })
            .is_empty()
    );
}

#[test]
fn normalizes_defender_uid_reservations() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.defender_reservation_count(&sonettobuf::Fight {
            battle_id: Some(9_000_161),
            ..Default::default()
        }),
        2
    );
    assert_eq!(
        catalog.defender_reservation_count(&sonettobuf::Fight {
            episode_id: Some(90_001_601),
            ..Default::default()
        }),
        0
    );
    assert_eq!(
        catalog.defender_reservation_count(&sonettobuf::Fight {
            battle_id: Some(i32::MAX),
            ..Default::default()
        }),
        0
    );
}

#[test]
fn normalizes_boss_rush_target_models() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.boss_rush_target_models(12_800_101, 1_014_201),
        Some(vec![10_142_011])
    );
    assert_eq!(catalog.boss_rush_target_models(12_800_101, 1_014_202), None);
    assert_eq!(catalog.boss_rush_target_models(-1, -1), None);
}

#[test]
fn normalizes_battle_outcome_rules() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.battle_max_round(1_211), Some(20));
    assert_eq!(catalog.battle_win_target_model(1_211), Some(121_103));
    assert_eq!(catalog.battle_win_target_model(1_001), None);
    assert_eq!(catalog.battle_max_round(-1), None);
    assert_eq!(catalog.battle_win_target_model(-1), None);
}

#[test]
fn normalizes_grouped_careers() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.careers(101), vec![1, 2]);
    assert_eq!(catalog.careers(1), vec![1]);
    assert_eq!(catalog.careers(-1), vec![-1]);
}

#[test]
fn normalizes_entity_identity_metadata() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.model_label(900016101), 7);
    assert_eq!(catalog.model_label(-1), 0);
    assert_eq!(catalog.entity_damage_type(3081, Some(1)), 1);
    assert_eq!(catalog.entity_damage_type(3081, Some(2)), 0);
    assert_eq!(catalog.entity_damage_type(900016101, Some(2)), 1);
    assert_eq!(catalog.entity_damage_type(-1, Some(1)), 0);
    assert_eq!(catalog.entity_damage_type(-1, Some(2)), 0);
}

#[test]
fn normalizes_monster_resistances() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.monster_resistances(10_212_111),
        Some(MonsterResistances {
            dizzy: 1000,
            frozen: 2000,
            seal: 1000,
            cant_get_exskill: 1000,
            control_resilience: 2000,
            ..Default::default()
        })
    );
    assert_eq!(catalog.monster_resistances(-1), None);
    assert_eq!(catalog.monster_resistances(900_016_101), None);
}

#[test]
fn normalizes_entity_base_technic() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.entity_base_technic(3081, 1, Some(1), 99), 273);
    assert_eq!(catalog.entity_base_technic(3081, 2, Some(1), 99), 99);
    assert_eq!(catalog.entity_base_technic(3081, 1, Some(2), 99), 99);
    assert_eq!(catalog.entity_base_technic(-1, 1, Some(1), 99), 99);
}

#[test]
fn normalizes_entity_battle_tags() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(catalog.entity_battle_tags(3081, 0, 0), vec![101, 105, 117]);
    assert_eq!(
        catalog.entity_battle_tags(3081, 308101, 1),
        vec![102, 114, 116]
    );
    assert_eq!(
        catalog.entity_battle_tags(3081, 308101, 0),
        vec![101, 105, 117]
    );
    assert!(catalog.entity_battle_tags(-1, -1, -1).is_empty());
}

#[test]
fn normalizes_entity_ex_attributes() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert_eq!(
        catalog.entity_ex_attributes(3081, Some(1), Some(1)),
        EntityExAttributes {
            crit_rate: 0,
            crit_resist: 0,
            crit_dmg: 1300,
            crit_def: 0,
            add_dmg: 0,
            drop_dmg: 0,
        }
    );
    assert_eq!(
        catalog.entity_ex_attributes(30110801, Some(170), Some(2)),
        EntityExAttributes {
            crit_rate: 48,
            crit_resist: 0,
            crit_dmg: 1200,
            crit_def: 387,
            add_dmg: 140,
            drop_dmg: 318,
        }
    );
    assert_eq!(
        catalog.entity_ex_attributes(-1, None, Some(1)),
        EntityExAttributes::default()
    );
    assert_eq!(
        catalog.entity_ex_attributes(-1, None, Some(2)),
        EntityExAttributes::default()
    );
}

#[test]
fn normalizes_card_enchant_compatibility() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());

    assert!(catalog.card_enchant_excluded_ids(10_001).is_empty());
    assert_eq!(
        catalog.card_enchant_rejected_ids(10_001),
        vec![10_002, 10_005, 10_007, 10_010, 10_011]
    );
    assert_eq!(catalog.card_enchant_rejected_ids(10_006), vec![10_003]);
    assert!(catalog.card_enchant_rejected_ids(-1).is_empty());
    assert_eq!(
        catalog.card_enchant_current_hp_loss_permille(10_002),
        Some(100)
    );
    assert_eq!(catalog.card_enchant_current_hp_loss_permille(10_010), None);
    assert_eq!(catalog.card_enchant_current_hp_loss_permille(-1), None);
}

#[test]
fn normalizes_card_skill_rank() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let card = |skill_id, card_effect| sonettobuf::CardInfo {
        skill_id,
        card_effect,
        ..Default::default()
    };

    assert_eq!(catalog.skill_rank(31_345_111), 1);
    assert_eq!(catalog.skill_rank(-1), 0);
    assert_eq!(catalog.card_skill_rank(&card(Some(31_345_111), Some(9))), 1);
    assert_eq!(catalog.card_skill_rank(&card(Some(-1), Some(9))), 9);
    assert_eq!(catalog.card_skill_rank(&card(None, Some(9))), 9);
}

#[test]
fn resolves_device_card_weights_from_the_selected_device() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let entity = |model_id, ex_skill_level, destiny_stone| sonettobuf::FightEntityInfo {
        model_id: Some(model_id),
        ex_skill_level: Some(ex_skill_level),
        destiny_stone: Some(destiny_stone),
        ..Default::default()
    };

    assert_eq!(
        catalog.device_card_weights(&entity(3_149, 0, 0)),
        vec![
            (31_446_011, 2),
            (31_446_012, 2),
            (31_446_021, 2),
            (31_446_022, 2),
            (31_490_201, 1),
            (31_490_211, 1),
        ]
    );
    assert_eq!(
        catalog.device_card_weights(&entity(3_144, 1, 0)),
        vec![(31_446_011, 2), (31_446_012, 2), (31_447_001, 1)]
    );
    assert_eq!(
        catalog.device_card_weights(&entity(3_025, 3, 302_502)),
        vec![(31_446_021, 3), (31_446_022, 1), (31_447_002, 1)]
    );
    assert!(catalog.device_card_weights(&entity(-1, 0, 0)).is_empty());
}

#[test]
fn resolves_selected_destiny_device_before_normal_and_base_devices() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let entity = sonettobuf::FightEntityInfo {
        model_id: Some(3025),
        ex_skill_level: Some(2),
        destiny_stone: Some(302502),
        destiny_rank: Some(4),
        ..Default::default()
    };

    assert_eq!(
        catalog.conduit_device(&entity).unwrap().unwrap(),
        vec![
            vec![crate::engine::manager::conduit::ConduitSkill {
                skill_id: 302524112,
                cost_type: 2,
                cost_value: 1,
                is_stopped: false,
            }],
            vec![crate::engine::manager::conduit::ConduitSkill {
                skill_id: 302514212,
                cost_type: 2,
                cost_value: 1,
                is_stopped: false,
            }],
            vec![crate::engine::manager::conduit::ConduitSkill {
                skill_id: 302504312,
                cost_type: 999,
                cost_value: 0,
                is_stopped: false,
            }],
        ]
    );
}

#[test]
fn resolves_normal_exact_skill_level_device_before_base_device() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let entity = sonettobuf::FightEntityInfo {
        model_id: Some(3144),
        ex_skill_level: Some(2),
        ..Default::default()
    };

    assert_eq!(
        catalog
            .conduit_device(&entity)
            .unwrap()
            .unwrap()
            .into_iter()
            .flatten()
            .map(|skill| skill.skill_id)
            .collect::<Vec<_>>(),
        vec![31441111, 31441121, 31441131]
    );
}

#[test]
fn resolves_max_coppelia_device_and_its_actual_skills() {
    crate::test_support::init_config();
    let game = crate::test_support::game_data();

    assert_eq!(
        configured_conduit_device_id(game, 3144, 5, 0),
        Some(31440051)
    );
    assert_eq!(
        configured_conduit_skill_ids(game, 3144, 5, 0).unwrap(),
        Some(vec![31444111, 31441121, 31445131])
    );
}

#[test]
fn ignores_zero_device_from_exact_destiny_skill_level() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let entity = sonettobuf::FightEntityInfo {
        model_id: Some(3081),
        ex_skill_level: Some(2),
        destiny_stone: Some(308101),
        destiny_rank: Some(4),
        ..Default::default()
    };

    assert!(catalog.conduit_device(&entity).unwrap().is_none());
}

#[test]
fn resolves_base_device_when_no_exact_skill_level_device_exists() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let entity = sonettobuf::FightEntityInfo {
        model_id: Some(3149),
        ex_skill_level: Some(0),
        ..Default::default()
    };

    assert_eq!(
        catalog
            .conduit_device(&entity)
            .unwrap()
            .unwrap()
            .into_iter()
            .flatten()
            .map(|skill| skill.skill_id)
            .collect::<Vec<_>>(),
        vec![31490111, 31490121, 31490131, 31490141, 31490151]
    );
}

#[test]
fn trial_skill_groups_require_exact_positive_identity() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let (configured, _) =
        crate::engine::entity::builder::EntityBuilder::trial(116_385_001, 42, 0, 1).unwrap();

    assert_eq!(
        catalog.trial_skill_groups(116_385_001, 3_149),
        Some(ConfiguredSkillGroups {
            group1: configured.skill_group1,
            group2: configured.skill_group2,
        })
    );
    assert_eq!(catalog.trial_skill_groups(0, 3_149), None);
    assert_eq!(catalog.trial_skill_groups(116_385_001, 999), None);
}

#[test]
fn fight_skill_catalog_includes_missing_exact_trial_groups() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let configured = catalog.trial_skill_groups(116_385_001, 3_149).unwrap();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(-1),
                model_id: Some(3_149),
                trial_id: Some(116_385_001),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let effects = catalog.skill_effects_for_fight(&fight);

    assert!(
        configured
            .group1
            .iter()
            .all(|skill_id| effects.get(*skill_id).is_some())
    );
    assert!(
        configured
            .group2
            .iter()
            .all(|skill_id| effects.get(*skill_id).is_some())
    );
}

#[test]
fn rejects_unsupported_lingering_glow_attribute_buff() {
    crate::test_support::init_config();
    let game_data = crate::test_support::game_data();

    assert_eq!(
        lingering_glow_attribute_origin(game_data, "1053#201#0#1000000"),
        None
    );
    assert!(lingering_glow_attribute_origin(game_data, "1053#201#5#1000000#10000").is_some());
}
