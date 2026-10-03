#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageFormula {
    StandardSkill,
    AdditionalDamage,
    CreditedSourceAdditional,
    AttributeReplacementAdditional,
    MaxHpAdditionalDamage,
    AttributeReplacement,
    HpSkillDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageFormulaRules {
    pub applies_career: bool,
    pub applies_target_base_critical_defense: bool,
    pub applies_target_buff_critical_defense: bool,
    pub includes_trigger_history: bool,
    pub combines_action_with_might: bool,
    pub nets_final_damage_in_regular: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlyPoisonFormulaInput {
    pub source_attack: i32,
    pub base_rate: i32,
    pub stacks: i32,
    pub compound_count: i32,
    pub compound_rate: i32,
}

impl DamageFormula {
    pub const fn rules(self) -> DamageFormulaRules {
        match self {
            Self::AdditionalDamage => DamageFormulaRules {
                applies_career: false,
                applies_target_base_critical_defense: true,
                applies_target_buff_critical_defense: true,
                includes_trigger_history: false,
                combines_action_with_might: false,
                nets_final_damage_in_regular: false,
            },
            Self::CreditedSourceAdditional | Self::AttributeReplacementAdditional => {
                DamageFormulaRules {
                    applies_career: true,
                    applies_target_base_critical_defense: true,
                    applies_target_buff_critical_defense: true,
                    includes_trigger_history: false,
                    combines_action_with_might: false,
                    nets_final_damage_in_regular: false,
                }
            }
            Self::MaxHpAdditionalDamage => DamageFormulaRules {
                applies_career: true,
                applies_target_base_critical_defense: true,
                applies_target_buff_critical_defense: false,
                includes_trigger_history: false,
                combines_action_with_might: false,
                nets_final_damage_in_regular: true,
            },
            Self::AttributeReplacement => DamageFormulaRules {
                applies_career: true,
                applies_target_base_critical_defense: true,
                applies_target_buff_critical_defense: true,
                includes_trigger_history: true,
                combines_action_with_might: true,
                nets_final_damage_in_regular: false,
            },
            Self::StandardSkill | Self::HpSkillDamage => DamageFormulaRules {
                applies_career: true,
                applies_target_base_critical_defense: true,
                applies_target_buff_critical_defense: true,
                includes_trigger_history: true,
                combines_action_with_might: false,
                nets_final_damage_in_regular: false,
            },
        }
    }

    pub const fn compose_might_and_action(self, might: i32, action: i32) -> (i32, i32) {
        if self.rules().combines_action_with_might {
            (might.saturating_add(action).saturating_sub(1000), 1000)
        } else {
            (might, action)
        }
    }

    pub const fn split_final_damage(self, delta: i32) -> (i32, i32) {
        if self.rules().nets_final_damage_in_regular {
            (delta, 0)
        } else {
            (0, delta)
        }
    }
}

pub fn attribute_scaled_damage(attribute: i32, rate: i32, stacks: i32) -> i32 {
    scale_permille_stacks(attribute, rate, stacks)
}

pub fn scale_permille(value: i32, rate: i32) -> i32 {
    scale_permille_stacks(value, rate, 1)
}

pub fn scale_permille_stacks(value: i32, rate: i32, stacks: i32) -> i32 {
    clamp_i32(
        i128::from(value.max(0)) * i128::from(rate.max(0)) * i128::from(stacks.max(0)) / 1_000,
    )
}

pub fn deadly_poison_damage(input: DeadlyPoisonFormulaInput) -> i32 {
    let base = attribute_scaled_damage(input.source_attack, input.base_rate, input.stacks);
    let compound_multiplier = 1_000_i128
        + i128::from(input.compound_count.max(0)) * i128::from(input.compound_rate.max(0));
    clamp_i32(i128::from(base) * compound_multiplier / 1_000)
}

pub fn butterfly_damage(
    recorded_damage: i32,
    base_rate: i32,
    gauge_raw: i32,
    raw_step: i32,
    rate_per_step: i32,
    raw_cap: i32,
) -> i32 {
    if raw_step <= 0 || raw_cap < 0 {
        return 0;
    }
    let rate = base_rate.max(0).saturating_add(
        (gauge_raw.clamp(0, raw_cap) / raw_step).saturating_mul(rate_per_step.max(0)),
    );
    attribute_scaled_damage(recorded_damage, rate, 1)
}

fn clamp_i32(value: i128) -> i32 {
    value.clamp(0, i128::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
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
}
