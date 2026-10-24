use super::*;

#[test]
fn fractional_minimum_is_limited_to_noncritical_standard_skills() {
    assert_eq!(
        minimum_damage_for_direct_formula(DamageFormula::StandardSkill, false, 894),
        MinimumDamage::AttackTenth
    );
    assert_eq!(
        minimum_damage_for_direct_formula(DamageFormula::StandardSkill, true, 894),
        MinimumDamage::Absolute(89)
    );
    for formula in [
        DamageFormula::AdditionalDamage,
        DamageFormula::ResolvedHitProportionalAdditional,
        DamageFormula::CreditedSourceAdditional,
        DamageFormula::AttributeReplacementAdditional,
        DamageFormula::MaxHpAdditionalDamage,
        DamageFormula::AttributeReplacement,
        DamageFormula::HpSkillDamage,
    ] {
        assert_eq!(
            minimum_damage_for_direct_formula(formula, false, 894),
            MinimumDamage::Absolute(89)
        );
    }
}

#[test]
fn formula_truncates_once_then_applies_crit() {
    let inputs = DamageInputs {
        kind: DamageKind::Reality,
        attack: 1380,
        defense: 200,
        defense_multiplier: 1000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(138),
        base_rate: 4500,
        added_rate: 0,
        career: 1300,
        regular: 300,
        attack_local: 1000,
        might: 1162,
        action: 1000,
        genesis: 1000,
        final_rate: 1000,
        poison_multiplier: None,
        crit_multiplier: 1850,
        is_crit: true,
    };

    assert_eq!(calculate_damage(inputs), 4451);
    assert_eq!(
        calculate_damage(DamageInputs {
            is_crit: false,
            ..inputs
        }),
        2406
    );
}
#[test]
fn attack_local_bonus_is_a_separate_multiplier() {
    let base = DamageInputs {
        kind: DamageKind::Reality,
        attack: 1380,
        defense: 200,
        defense_multiplier: 1000,
        penetration: 0,
        minimum: MinimumDamage::Absolute(138),
        base_rate: 1000,
        added_rate: 0,
        career: 1300,
        regular: 300,
        attack_local: 1000,
        might: 1000,
        action: 1000,
        genesis: 1000,
        final_rate: 1000,
        poison_multiplier: None,
        crit_multiplier: 2126,
        is_crit: true,
    };

    assert_eq!(calculate_damage(base), 978);
    assert_eq!(
        calculate_damage(DamageInputs {
            attack_local: 1500,
            ..base
        }),
        1467
    );
}
