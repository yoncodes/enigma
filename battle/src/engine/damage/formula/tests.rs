use super::{
    DamageFormula, DeadlyPoisonFormulaInput, attribute_scaled_damage, butterfly_damage,
    deadly_poison_damage, scale_permille, scale_permille_stacks,
};

#[test]
fn formula_owns_damage_policy() {
    assert!(!DamageFormula::AdditionalDamage.rules().applies_career);
    assert!(
        DamageFormula::CreditedSourceAdditional
            .rules()
            .applies_career
    );
    assert!(
        DamageFormula::AttributeReplacementAdditional
            .rules()
            .applies_career
    );
    assert!(DamageFormula::MaxHpAdditionalDamage.rules().applies_career);
    assert!(
        DamageFormula::AdditionalDamage
            .rules()
            .applies_target_base_critical_defense
    );
    assert!(
        DamageFormula::AdditionalDamage
            .rules()
            .applies_target_buff_critical_defense
    );
    assert!(
        DamageFormula::MaxHpAdditionalDamage
            .rules()
            .applies_target_base_critical_defense
    );
    assert!(
        !DamageFormula::MaxHpAdditionalDamage
            .rules()
            .applies_target_buff_critical_defense
    );
    assert_eq!(
        DamageFormula::AttributeReplacement.compose_might_and_action(1_120, 1_500),
        (1_620, 1_000)
    );
    assert_eq!(
        DamageFormula::HpSkillDamage.compose_might_and_action(1_120, 1_500),
        (1_120, 1_500)
    );
    assert_eq!(
        DamageFormula::MaxHpAdditionalDamage.split_final_damage(-100),
        (-100, 0)
    );
    assert_eq!(
        DamageFormula::HpSkillDamage.split_final_damage(-100),
        (0, -100)
    );
    assert!(
        DamageFormula::StandardSkill
            .rules()
            .truncates_signed_defense_rate_delta
    );
    assert!(
        !DamageFormula::AdditionalDamage
            .rules()
            .truncates_signed_defense_rate_delta
    );
}

#[test]
fn fixed_point_scaling_widens_before_multiplication() {
    assert_eq!(scale_permille(3_000_000, 1_000), 3_000_000);
    assert_eq!(scale_permille_stacks(3_000_000, 1_000, 10), 30_000_000);
    assert_eq!(scale_permille(i32::MAX, i32::MAX), i32::MAX);
}

#[test]
fn attribute_scaling_keeps_one_fraction_boundary() {
    assert_eq!(attribute_scaled_damage(1_441, 40, 8), 461);
}

#[test]
fn deadly_poison_compounds_after_base_stack_damage() {
    assert_eq!(
        deadly_poison_damage(DeadlyPoisonFormulaInput {
            source_attack: 1_000,
            base_rate: 100,
            stacks: 2,
            compound_count: 3,
            compound_rate: 50,
        }),
        230
    );
}

#[test]
fn butterfly_rate_is_derived_from_capped_raw_gauge_progress() {
    assert_eq!(butterfly_damage(2_000, 100, 250, 50, 25, 200), 400);
}
