use super::*;
use crate::engine::{
    manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn settlement_calculates_once_and_emits_one_hp_command() {
    let formula = DamageFormulaInput {
        kind: DamageKind::Reality,
        attack: 1_000,
        defense: 200,
        defense_multiplier: 1_000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(100),
        base_rate: 1_000,
        added_rate: 0,
        career: 1_000,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_500,
        is_crit: true,
    };
    let command = DamageSettlement {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 1,
        target_uid: 2,
        config_effect: 3,
        effect_kind: DamageEffectKind::Critical,
        hurt: HurtInfoData {
            from_uid: 1,
            is_crit: true,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 0,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
        formula,
    }
    .command()
    .unwrap();

    assert_eq!(calculate(formula), 1_200);
    assert!(matches!(
        command,
        HpCommand::Damage(HpDamage {
            amount: 1_200,
            effect_kind: DamageEffectKind::Critical,
            ..
        })
    ));
}

#[test]
fn trace_preserves_fractional_damage_until_final_settlement() {
    let input = DamageFormulaInput {
        kind: DamageKind::Mental,
        attack: 1_923,
        defense: 567,
        defense_multiplier: 1_000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(192),
        base_rate: 1_300,
        added_rate: 0,
        career: 1_000,
        regular: 903,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_346,
        is_crit: true,
    };

    let trace = calculate_with_trace(input);

    assert_eq!(trace, calculate_with_trace(input));
    assert_eq!(trace.defense.output(), 1_356);
    assert_eq!(trace.skill_rate.output, 1_762);
    assert_eq!(trace.combined_multipliers.multipliers.regular, 903);
    assert_eq!(trace.skill_rate.career, 1_000);
    assert_eq!(trace.combined_multipliers.output, 1_591);
    assert_eq!(trace.critical.output, 2_142);
    assert_eq!(trace.amount, calculate(input));
    assert!(matches!(
        trace.stages(),
        [
            Some(DamageStageTrace::Defense(_)),
            Some(DamageStageTrace::SkillRate(_)),
            Some(DamageStageTrace::CombinedMultipliers(_)),
            None,
            Some(DamageStageTrace::Critical(_)),
        ]
    ));
}

#[test]
fn critical_multiplier_preserves_conversion_remainder_until_settlement() {
    let trace = pipeline::calculate_with_trace_for_version_and_remainder(
        DamageFormulaInput {
            kind: DamageKind::Mental,
            attack: 1_860,
            defense: 700,
            defense_multiplier: 1_000,
            penetration: 0,
            minimum: MinimumDamage::Absolute(186),
            base_rate: 11_112,
            added_rate: 0,
            career: 1_000,
            regular: 595,
            attack_local: 1_000,
            might: 1_050,
            action: 1_000,
            genesis: 1_000,
            final_rate: 1_000,
            poison_multiplier: None,
            crit_multiplier: 1_820,
            is_crit: true,
        },
        7,
        500,
    );

    assert_eq!(trace.amount, 14_660);
}

#[test]
fn critical_damage_uses_the_capture_proven_minimum_multiplier() {
    let trace = pipeline::calculate_with_trace_for_version(
        DamageFormulaInput {
            kind: DamageKind::Reality,
            attack: 2_210,
            defense: 1_200,
            defense_multiplier: 1_000,
            penetration: 0,
            minimum: MinimumDamage::Absolute(221),
            base_rate: 300,
            added_rate: 0,
            career: 1_300,
            regular: 1_675,
            attack_local: 1_000,
            might: 150,
            action: 1_000,
            genesis: 1_000,
            final_rate: 1_000,
            poison_multiplier: None,
            crit_multiplier: 82,
            is_crit: true,
        },
        7,
    );

    assert_eq!(trace.critical.multiplier, 1_100);
    assert_eq!(trace.amount, 108);
}

#[test]
fn poison_has_its_capture_proven_floor_before_critical() {
    let mut input = DamageFormulaInput::genesis(554, 1_000, 1_150);
    input.poison_multiplier = Some(1_469);
    input.crit_multiplier = 1_301;
    input.is_crit = true;

    let trace = calculate_with_trace(input);

    assert_eq!(trace.combined_multipliers.output, 637);
    assert_eq!(trace.poison_multiplier.unwrap().output, 935);
    assert_eq!(trace.critical.output, 1_216);
    assert_eq!(trace.amount, 1_216);
}

#[test]
fn career_advantage_applies_only_to_the_configured_base_rate() {
    let trace = calculate_with_trace(DamageFormulaInput {
        kind: DamageKind::Reality,
        attack: 1_000,
        defense: 0,
        defense_multiplier: 1_000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(100),
        base_rate: 1_000,
        added_rate: 1_000,
        career: 1_300,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_000,
        is_crit: false,
    });

    assert_eq!(trace.skill_rate.base_numerator, 1_300_000_000);
    assert_eq!(trace.skill_rate.added_numerator, 1_000_000_000);
    assert_eq!(trace.skill_rate.output, 2_300);
    assert_eq!(trace.amount, 2_300);
}

#[test]
fn genesis_uses_the_same_pipeline_and_explicitly_skips_defense() {
    let trace = calculate_with_trace(DamageFormulaInput {
        kind: DamageKind::Genesis,
        attack: 2_025,
        defense: 999,
        defense_multiplier: 999,
        penetration: 999,
        minimum: MinimumDamage::Absolute(999),
        base_rate: 200,
        added_rate: 0,
        career: 1_000,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_000,
        is_crit: false,
    });

    assert_eq!(trace.defense, DefenseTrace::Skipped { input: 2_025 });
    assert_eq!(trace.amount, 405);
}

#[test]
fn genesis_constructor_shares_skill_and_modifier_stages() {
    let trace = calculate_with_trace(DamageFormulaInput::genesis(1_000, 500, 1_200));

    assert_eq!(trace.defense, DefenseTrace::Skipped { input: 1_000 });
    assert_eq!(trace.skill_rate.output, 500);
    assert_eq!(trace.combined_multipliers.multipliers.genesis, 1_200);
    assert_eq!(trace.combined_multipliers.output, 600);
    assert_eq!(trace.amount, 600);
}

#[test]
fn defense_trace_keeps_modifier_and_penetration_divisions_separate() {
    let trace = calculate_with_trace(DamageFormulaInput {
        kind: DamageKind::Reality,
        attack: 2_000,
        defense: 1_000,
        defense_multiplier: 800,
        penetration: 250,
        minimum: MinimumDamage::Absolute(200),
        base_rate: 1_000,
        added_rate: 0,
        career: 1_000,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_000,
        is_crit: false,
    });

    assert!(matches!(
        trace.defense,
        DefenseTrace::Applied {
            raw_defense: 1_000,
            modified_defense: 800,
            penetration: 250,
            effective_defense: 600,
            output: 1_400,
            ..
        }
    ));
    assert_eq!(trace.amount, 1_400);
}

#[test]
fn version_seven_preserves_effective_defense_until_final_settlement() {
    let input = |defense, career, regular, is_crit| DamageFormulaInput {
        kind: DamageKind::Reality,
        attack: 1_983,
        defense,
        defense_multiplier: 1_000,
        penetration: 920,
        minimum: MinimumDamage::Absolute(198),
        base_rate: 18_700,
        added_rate: 0,
        career,
        regular,
        attack_local: 1_000,
        might: 1_380,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_385,
        is_crit,
    };

    let cases = [
        (input(554, 1_000, 1_977, false), 98_924, 98_908),
        (input(471, 1_000, 2_027, false), 101_792, 101_757),
        (input(436, 1_300, 2_027, true), 183_560, 183_477),
    ];
    for (input, legacy, current) in cases {
        assert_eq!(calculate_with_trace_for_version(input, 6).amount, legacy);
        assert_eq!(calculate_with_trace_for_version(input, 7).amount, current);
    }
}

#[test]
fn version_seven_defense_attribute_rate_truncates_the_signed_delta() {
    for (defense, multiplier, legacy, current) in [(422, 600, 253, 254), (893, 1_200, 1_072, 1_071)]
    {
        let input = DamageFormulaInput {
            kind: DamageKind::Mental,
            attack: 2_065,
            defense,
            defense_multiplier: multiplier,
            penetration: 0,
            minimum: MinimumDamage::Absolute(206),
            base_rate: 1_000,
            added_rate: 0,
            career: 1_000,
            regular: 1_000,
            attack_local: 1_000,
            might: 1_000,
            action: 1_000,
            genesis: 1_000,
            final_rate: 1_000,
            poison_multiplier: None,
            crit_multiplier: 1_000,
            is_crit: false,
        };

        for (version, expected) in [(6, legacy), (7, current)] {
            let trace = pipeline::calculate_with_trace_for_formula_and_remainder(
                input,
                version,
                0,
                DamageFormula::StandardSkill,
            );
            assert!(matches!(
                trace.defense,
                DefenseTrace::Applied {
                    modified_defense,
                    effective_defense,
                    ..
                } if modified_defense == expected && effective_defense == expected
            ));
        }
    }
}

#[test]
fn version_seven_non_standard_damage_keeps_nearest_defense_rate_rounding() {
    let input = DamageFormulaInput {
        kind: DamageKind::Mental,
        attack: 2_065,
        defense: 422,
        defense_multiplier: 600,
        penetration: 0,
        minimum: MinimumDamage::Absolute(206),
        base_rate: 1_000,
        added_rate: 0,
        career: 1_000,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_000,
        is_crit: false,
    };

    let trace = pipeline::calculate_with_trace_for_formula_and_remainder(
        input,
        7,
        0,
        DamageFormula::AdditionalDamage,
    );
    assert_eq!(trace.amount, 1_812);
    assert!(matches!(
        trace.defense,
        DefenseTrace::Applied {
            modified_defense: 253,
            ..
        }
    ));
}

#[test]
fn defense_clamps_to_the_configured_minimum_damage() {
    let trace = calculate_with_trace(DamageFormulaInput {
        kind: DamageKind::Reality,
        attack: 100,
        defense: 500,
        defense_multiplier: 1_000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(10),
        base_rate: 1_000,
        added_rate: 0,
        career: 1_000,
        regular: 1_000,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_000,
        is_crit: false,
    });

    assert_eq!(trace.defense.output(), 10);
    assert_eq!(trace.amount, 10);
}

#[test]
fn version_seven_preserves_fractional_minimum_damage_until_settlement() {
    let input = DamageFormulaInput {
        kind: DamageKind::Mental,
        attack: 894,
        defense: 827,
        defense_multiplier: 1_000,
        penetration: 0,
        minimum: MinimumDamage::AttackTenth,
        base_rate: 2_000,
        added_rate: 0,
        career: 1_000,
        regular: 841,
        attack_local: 1_000,
        might: 1_000,
        action: 1_000,
        genesis: 1_000,
        final_rate: 1_000,
        poison_multiplier: None,
        crit_multiplier: 1_318,
        is_crit: false,
    };

    let legacy = calculate_with_trace_for_version(input, 6);
    assert!(matches!(
        legacy.defense,
        DefenseTrace::Applied {
            minimum: 89,
            minimum_numerator: 89_000,
            minimum_denominator: 1_000,
            ..
        }
    ));
    assert_eq!(legacy.amount, 149);
    let current = calculate_with_trace_for_version(input, 7);
    assert!(matches!(
        current.defense,
        DefenseTrace::Applied {
            minimum: 89,
            minimum_numerator: 89_400,
            minimum_denominator: 1_000,
            ..
        }
    ));
    assert_eq!(current.amount, 150);
}

#[test]
fn captured_defense_matrix_preserves_hp_replacement_formula() {
    for (defense, expected) in [(448, 449), (869, 297)] {
        let input = DamageFormulaInput {
            kind: DamageKind::Reality,
            attack: 1_696,
            defense,
            defense_multiplier: 1_000,
            penetration: 0,
            minimum: MinimumDamage::Absolute(169),
            base_rate: 1_200,
            added_rate: 0,
            career: 1_000,
            regular: 300,
            attack_local: 1_000,
            might: 1_000,
            action: 1_000,
            genesis: 1_000,
            final_rate: 1_000,
            poison_multiplier: None,
            crit_multiplier: 1_000,
            is_crit: false,
        };

        assert_eq!(calculate(input), expected);
    }
}

#[test]
fn captured_direct_hits_keep_fractional_precision() {
    let cases = [
        (1_848, 567, 800, 1_000, 818, 1_301, true, 1_090),
        (1_848, 567, 800, 1_300, 915, 1_387, false, 1_218),
        (2_240, 567, 1_300, 1_000, 703, 1_346, true, 2_057),
    ];
    for (attack, defense, rate, career, regular, crit, is_crit, expected) in cases {
        assert_eq!(
            calculate(DamageFormulaInput {
                kind: DamageKind::Mental,
                attack,
                defense,
                defense_multiplier: 1_000,
                penetration: 0,
                minimum: MinimumDamage::Absolute(attack / 10),
                base_rate: rate,
                added_rate: 0,
                career,
                regular,
                attack_local: 1_000,
                might: 1_000,
                action: 1_000,
                genesis: 1_000,
                final_rate: 1_000,
                poison_multiplier: None,
                crit_multiplier: crit,
                is_crit,
            }),
            expected
        );
    }
}
