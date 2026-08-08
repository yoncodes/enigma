use super::*;

#[test]
fn stat_inputs_use_only_normalized_battle_build_data() {
    let hero = HeroBuildInput {
        hero_id: 3127,
        level: 121,
        rank: 4,
        destiny_rank: 3,
        talent: 10,
        talent_style: 2,
        talent_placements: vec![11, 12],
        ..Default::default()
    };
    let equip = EquipmentBuildInput {
        equip_id: 1502,
        level: 50,
        break_level: 4,
        ..Default::default()
    };

    assert_eq!(
        StatInputs::from_build_input(&hero, Some(&equip)),
        StatInputs {
            hero_id: 3127,
            level: 121,
            rank: 4,
            destiny_rank: 3,
            equip_id: 1502,
            equip_level: 50,
            equip_break_level: 4,
            talent: 10,
            talent_style: 2,
            talent_placements: vec![11, 12],
        }
    );
}

#[test]
fn configured_pickles_stats_are_generated_from_build_inputs() {
    crate::test_support::init_config();
    let stats = Stats::build(&StatInputs {
        hero_id: 3063,
        level: 180,
        rank: 4,
        destiny_rank: 4,
        equip_id: 1523,
        equip_level: 60,
        talent: 10,
        talent_style: 0,
        talent_placements: vec![10, 10, 61, 15, 12, 10, 12, 16, 17, 13, 14, 17, 19, 19],
        ..Default::default()
    });
    assert_eq!(
        (stats.hp, stats.atk, stats.def, stats.mdef, stats.technic),
        (10914, 1874, 831, 927, 305)
    );
    assert_eq!(
        (
            stats.cri,
            stats.recri,
            stats.cri_def,
            stats.add_dmg,
            stats.drop_dmg
        ),
        (365, 190, 50, 165, 155)
    );
    assert_eq!(stats.reuse_dmg, 100);
    assert_eq!(stats.cri_dmg, 1360);
}

#[test]
fn rank_comes_from_character_rank_level_caps() {
    crate::test_support::init_config();
    assert_eq!(rank_from_level(3063, 30), 1);
    assert_eq!(rank_from_level(3063, 31), 2);
    assert_eq!(rank_from_level(3063, 71), 3);
    assert_eq!(rank_from_level(3063, 121), 4);
}

#[test]
fn monster_hidden_stats_follow_the_model_template_curve() {
    crate::test_support::init_config();

    let stats = monster_instance_ex_stats(30111001, 180).unwrap();
    assert_eq!(
        (
            stats.cri,
            stats.recri,
            stats.cri_dmg,
            stats.cri_def,
            stats.add_dmg,
            stats.drop_dmg,
        ),
        (217, 0, 1290, 135, 416, 95)
    );
}

#[test]
fn destiny_poison_rate_uses_unlocked_config_slots() {
    crate::test_support::init_config();

    assert_eq!(destiny_poison_add_rate(3009, 4), 100);
}

#[test]
fn battle_balance_applies_configured_stat_floors() {
    crate::test_support::init_config();

    let inputs = BattleBalance::parse("140#12#60")
        .unwrap()
        .apply(StatInputs {
            hero_id: 3127,
            level: 121,
            rank: 4,
            equip_id: 1502,
            equip_level: 50,
            talent: 10,
            talent_placements: vec![1, 2, 3],
            ..Default::default()
        });
    let stats = Stats::build(&inputs);

    assert_eq!(
        (inputs.level, inputs.rank, inputs.talent, inputs.equip_level),
        (140, 4, 12, 60)
    );
    assert!(inputs.talent_placements.is_empty());
    assert_eq!(
        (stats.hp, stats.atk, stats.def, stats.mdef, stats.technic),
        (9628, 1737, 820, 858, 504)
    );
    assert_eq!(stats.cri, 325);
}

#[test]
fn battle_balance_rejects_unconfigured_shapes() {
    assert_eq!(BattleBalance::parse(""), None);
    assert_eq!(BattleBalance::parse("140#12"), None);
    assert_eq!(BattleBalance::parse("140#12#60#1"), None);
    assert_eq!(BattleBalance::parse("140#0#60"), None);
}
