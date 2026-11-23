use super::EntityBuilder;
use crate::engine::entity::{
    input::{EquipmentBuildInput, HeroBuildInput},
    stats::Stats,
};
use sonettobuf::PowerInfo;

#[test]
fn catalog_attached_builder_matches_the_legacy_entry_point() {
    crate::test_support::init_config();
    let hero = HeroBuildInput {
        uid: 20_000_001,
        user_id: 1,
        hero_id: 3127,
        skin: 312701,
        level: 121,
        rank: 4,
        ex_skill_level: 3,
        talent: 10,
        ..Default::default()
    };
    let equips = vec![EquipmentBuildInput {
        uid: 30_000_001,
        equip_id: 1502,
        level: 50,
        break_level: 1,
        refine_level: 1,
    }];
    let stats = Stats::build_for_loadout(&hero, &equips);
    let legacy = EntityBuilder::new(hero.clone(), 1, 1)
        .with_equips(equips.clone())
        .build()
        .unwrap();
    let explicit = EntityBuilder::new(hero, 1, 1)
        .with_catalog(crate::catalog::BattleCatalog::new(
            crate::test_support::game_data(),
        ))
        .with_equips(equips)
        .with_stats(stats)
        .build()
        .unwrap();

    assert_eq!(explicit, legacy);
    assert_eq!(explicit.equips[0].break_lv, None);
}

#[test]
fn device_power_trial_keeps_its_unique_skill_out_of_ex_skill() {
    crate::test_support::init_config();

    let legacy = EntityBuilder::trial(116385001, 10, 1, 1).unwrap();
    let explicit = EntityBuilder::configured_trial(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        116385001,
        10,
        1,
        1,
    )
    .unwrap();

    assert_eq!(explicit, legacy);
    let (entity, _) = explicit;

    assert_eq!(entity.model_id, Some(3149));
    assert_eq!(entity.ex_point_type, Some(4));
    assert_eq!(entity.ex_point_max, Some(100));
    assert_eq!(entity.ex_skill, Some(0));
}

#[test]
fn kassandra_lit_talents_exchange_the_kit_and_add_their_skills() {
    crate::test_support::init_config();
    let hero = HeroBuildInput {
        uid: 20_000_005,
        user_id: 1,
        hero_id: 3124,
        skin: 312402,
        level: 180,
        rank: 4,
        ex_skill_level: 5,
        talent: 10,
        extra_str: "1#11,12|2#21,22,23".to_owned(),
        ..Default::default()
    };

    let lit = EntityBuilder::new(hero.clone(), 1, 1).build().unwrap();
    assert_eq!(lit.skill_group1, vec![312451115, 312451125, 312451135]);
    assert_eq!(lit.skill_group2, vec![312431212, 312431222, 312431232]);
    assert_eq!(lit.ex_skill, Some(312451031));
    assert_eq!(
        lit.passive_skill,
        vec![
            31243141, 31240142, 31244144, 312451405, 312401454, 312401451, 312451452, 312401444,
            312401441, 312441440,
        ]
    );

    let unlit = EntityBuilder::new(
        HeroBuildInput {
            extra_str: String::new(),
            ..hero.clone()
        },
        1,
        1,
    )
    .build()
    .unwrap();
    assert_eq!(unlit.skill_group1, vec![31243111, 31243112, 31243113]);
    assert_eq!(unlit.ex_skill, Some(31242103));

    // Talent 13 upgrades the passive talent 12 adds.
    let upgraded = EntityBuilder::new(
        HeroBuildInput {
            extra_str: "1#11,12,13".to_owned(),
            ..hero
        },
        1,
        1,
    )
    .build()
    .unwrap();
    assert!(upgraded.passive_skill.contains(&312461452));
    assert!(!upgraded.passive_skill.contains(&312451452));
}

#[test]
fn ezio_equipped_gear_replaces_the_kit_and_adds_its_passives() {
    crate::test_support::init_config();
    let hero = HeroBuildInput {
        uid: 20_000_004,
        user_id: 1,
        hero_id: 3123,
        skin: 312302,
        level: 180,
        rank: 4,
        ex_skill_level: 5,
        talent: 10,
        extra_str: "1001#2003".to_owned(),
        ..Default::default()
    };

    let geared = EntityBuilder::new(hero.clone(), 1, 1).build().unwrap();
    assert_eq!(geared.skill_group1, vec![312301142, 312301152, 312301162]);
    assert_eq!(geared.skill_group2, vec![312301242, 312301252, 312301262]);
    assert_eq!(geared.ex_skill, Some(312301313));
    assert!(geared.passive_skill.ends_with(&[312301701, 312300011]));

    let plain = EntityBuilder::new(
        HeroBuildInput {
            extra_str: String::new(),
            ..hero
        },
        1,
        1,
    )
    .build()
    .unwrap();
    assert_ne!(plain.skill_group1, geared.skill_group1);
    assert!(!plain.passive_skill.contains(&312300011));
}

#[test]
fn nautika_trial_gains_faith_only_from_insight_two() {
    crate::test_support::init_config();

    let (before, _) = EntityBuilder::trial(5280101, 10, 1, 1).unwrap();
    assert_eq!(
        (before.ex_point_type, before.ex_point_max),
        (Some(0), Some(5))
    );

    let (after, _) = EntityBuilder::trial(5280102, 10, 1, 1).unwrap();
    assert_eq!(
        (after.ex_point_type, after.ex_point_max),
        (Some(1), Some(8))
    );
}

#[test]
fn device_owned_roster_and_trial_omit_character_skill_groups() {
    crate::test_support::init_config();

    let roster = EntityBuilder::new(
        HeroBuildInput {
            uid: 20_000_002,
            user_id: 1,
            hero_id: 3144,
            skin: 314401,
            level: 180,
            rank: 5,
            ex_skill_level: 5,
            talent: 12,
            ..Default::default()
        },
        1,
        1,
    )
    .build()
    .unwrap();
    assert_eq!(roster.ex_point_type, Some(4));
    assert!(roster.skill_group1.is_empty());
    assert!(roster.skill_group2.is_empty());
    assert_eq!(roster.ex_skill, Some(0));

    let (trial, _) = EntityBuilder::trial(115380002, 20_000_003, 1, 1).unwrap();
    assert_eq!(trial.ex_point_type, Some(4));
    assert!(trial.skill_group1.is_empty());
    assert!(trial.skill_group2.is_empty());
    assert_eq!(trial.ex_skill, Some(0));
}

#[test]
fn selected_destiny_resource_replaces_character_resource() {
    crate::test_support::init_config();

    let entity = EntityBuilder::new(
        HeroBuildInput {
            hero_id: 3025,
            rank: 4,
            ex_skill_level: 0,
            destiny_stone: 302502,
            destiny_rank: 4,
            ..Default::default()
        },
        1,
        1,
    )
    .build()
    .unwrap();

    assert_eq!(entity.ex_point_type, Some(4));
    assert_eq!(entity.ex_point_max, Some(100));
    assert!(entity.skill_group1.is_empty());
    assert!(entity.skill_group2.is_empty());
    assert_eq!(entity.ex_skill, Some(0));
}

#[test]
fn selected_destiny_adds_its_configured_power_to_roster_and_trial() {
    crate::test_support::init_config();

    let hero = HeroBuildInput {
        hero_id: 3053,
        rank: 4,
        ex_skill_level: 3,
        ..Default::default()
    };
    let plain = EntityBuilder::new(hero.clone(), 1, 1).build().unwrap();
    let enhanced = EntityBuilder::new(
        HeroBuildInput {
            destiny_stone: 305301,
            destiny_rank: 1,
            ..hero
        },
        1,
        1,
    )
    .build()
    .unwrap();
    let (trial, _) = EntityBuilder::trial(9341003, 10, 1, 1).unwrap();

    assert!(plain.power_infos.is_empty());
    for entity in [enhanced, trial] {
        assert!(matches!(
            entity.power_infos.as_slice(),
            [PowerInfo {
                power_id: Some(1),
                num: Some(0),
                max: Some(5),
            }]
        ));
    }
}
