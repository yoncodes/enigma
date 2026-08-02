use super::*;

#[test]
fn action_bonus_nets_with_damage_taken_reduction_before_the_floor() {
    assert_eq!(regular_multiplier(935, 1_020), 915);
    assert_eq!(regular_multiplier(0, 600), 400);
    assert_eq!(regular_multiplier(0, 1_020), 300);
    assert_eq!(regular_multiplier(830, -525), 2_355);
}

#[test]
fn critical_technique_uses_the_targets_level_without_an_extra_scale() {
    assert_eq!(technique_bonus(600, 20, 150, 300, 5), 225);
}

#[test]
fn retribution_uses_one_baseline_and_sums_its_configured_bonuses() {
    let bonus = DamageRateTerm {
        opcode: 1,
        rate: 100,
        career_scaled: true,
        composition: crate::engine::damage::DamageRateComposition::RetributionLane,
    };
    let per_stack = DamageRateTerm {
        opcode: 1025,
        rate: 20,
        career_scaled: true,
        composition: crate::engine::damage::DamageRateComposition::RetributionLane,
    };

    assert_eq!(composed_damage_rates(1300, &[bonus]), (2400, 0));
    assert_eq!(composed_damage_rates(1300, &[bonus, per_stack]), (2420, 0));
    assert_eq!(
        composed_damage_rates(750, &[per_stack, per_stack]),
        (1790, 0)
    );
}

#[test]
fn linked_retribution_modifier_scales_the_existing_damage_producer() {
    let linked = DamageRateTerm {
        opcode: 1025,
        rate: 100,
        career_scaled: true,
        composition: crate::engine::damage::DamageRateComposition::ProducerMultiplier,
    };

    assert_eq!(composed_damage_rates(1200, &[linked]), (1320, 0));
    assert_eq!(composed_damage_rates(750, &[linked]), (825, 0));
}

#[test]
fn noncritical_heal_remains_owned_by_its_declaring_skill() {
    let definition = crate::engine::skill::behavior::registry::find_key(20016, "HealCantCrit")
        .expect("the exact heal behavior is registered");

    assert_eq!(
        definition.output_owner,
        crate::engine::skill::behavior::registry::OutputOwner::Skill
    );
}

#[test]
fn burn_applies_its_unstackable_healing_taken_reduction() {
    crate::test_support::init_config();
    let burn_type = super::heal::burn_type_id().expect("FightConst 29 defines Burn");
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![sonettobuf::BuffInfo {
                    buff_id: Some(burn_type),
                    duration: Some(0),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);

    assert!(managers.buff.has_active_buff_id_or_type(-1, burn_type));
    assert_eq!(super::heal::modified(1_000, 10, -1, &managers), 850);
}

#[test]
fn missing_hp_healing_uses_the_configured_base_bucket_and_cap() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![
                sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(5_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        buff_id: Some(31200124),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(500),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        buff_id: Some(31200124),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);

    assert_eq!(super::heal::modified(1_000, 10, 11, &managers), 1_575);
    assert_eq!(super::heal::modified(1_000, 10, 12, &managers), 1_800);
}
