#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Reality,
    Mental,
    Genesis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageFormulaInput {
    pub kind: DamageKind,
    pub attack: i32,
    pub defense: i32,
    pub defense_multiplier: i32,
    pub penetration: i32,
    pub minimum: i32,
    pub base_rate: i32,
    pub added_rate: i32,
    pub career: i32,
    pub regular: i32,
    pub attack_local: i32,
    pub might: i32,
    pub action: i32,
    pub genesis: i32,
    pub final_rate: i32,
    pub poison_multiplier: Option<i32>,
    pub crit_multiplier: i32,
    pub is_crit: bool,
}

impl DamageFormulaInput {
    pub fn genesis(attack: i32, base_rate: i32, genesis: i32) -> Self {
        Self {
            kind: DamageKind::Genesis,
            attack,
            defense: 0,
            defense_multiplier: 1_000,
            penetration: 0,
            minimum: 0,
            base_rate,
            added_rate: 0,
            career: 1_000,
            regular: 1_000,
            attack_local: 1_000,
            might: 1_000,
            action: 1_000,
            genesis,
            final_rate: 1_000,
            poison_multiplier: None,
            crit_multiplier: 1_000,
            is_crit: false,
        }
    }
}

pub fn calculate(input: DamageFormulaInput) -> i32 {
    calculate_with_trace(input).amount
}

fn greatest_common_divisor(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn multiply_reduced_fraction(
    (mut numerator, mut denominator): (i128, i128),
    (mut factor_numerator, mut factor_denominator): (i128, i128),
) -> (i128, i128) {
    let divisor = greatest_common_divisor(numerator, factor_denominator);
    numerator /= divisor;
    factor_denominator /= divisor;
    let divisor = greatest_common_divisor(factor_numerator, denominator);
    factor_numerator /= divisor;
    denominator /= divisor;
    (
        numerator * factor_numerator,
        denominator * factor_denominator,
    )
}

pub fn calculate_with_trace(input: DamageFormulaInput) -> DamageTrace {
    calculate_with_trace_for_version(input, 6)
}

pub fn calculate_with_trace_for_version(
    input: DamageFormulaInput,
    fight_version: i32,
) -> DamageTrace {
    calculate_with_trace_for_version_and_remainder(input, fight_version, 0)
}

pub(crate) fn calculate_with_trace_for_version_and_remainder(
    input: DamageFormulaInput,
    fight_version: i32,
    critical_multiplier_remainder: i32,
) -> DamageTrace {
    let preserve_effective_defense = fight_version == 7;
    let (defense, post_defense_fraction) = match input.kind {
        kind @ (DamageKind::Reality | DamageKind::Mental) => {
            let defense_multiplier = input.defense_multiplier.max(0);
            let penetration = input.penetration.clamp(0, 1000);
            let modified_defense =
                ((i64::from(input.defense.max(0)) * i64::from(defense_multiplier) + 500) / 1000)
                    .min(i64::from(i32::MAX)) as i32;
            let effective_numerator = i128::from(modified_defense) * i128::from(1000 - penetration);
            let effective_defense = (effective_numerator / 1000).min(i128::from(i32::MAX)) as i32;
            let post_defense_fraction = if preserve_effective_defense {
                let numerator = i128::from(input.attack) * 1000 - effective_numerator;
                (numerator.max(i128::from(input.minimum) * 1000), 1000)
            } else {
                (
                    i128::from((input.attack - effective_defense).max(input.minimum)),
                    1,
                )
            };
            let calculated_output = (post_defense_fraction.0 / post_defense_fraction.1) as i32;
            (
                DefenseTrace::Applied {
                    kind,
                    attack: input.attack,
                    raw_defense: input.defense,
                    defense_multiplier,
                    modified_defense,
                    penetration,
                    effective_defense,
                    minimum: input.minimum,
                    calculated_output,
                    output: calculated_output,
                },
                post_defense_fraction,
            )
        }
        DamageKind::Genesis => {
            let input = input.attack.max(0);
            (DefenseTrace::Skipped { input }, (i128::from(input), 1))
        }
    };
    let post_defense = (post_defense_fraction.0 / post_defense_fraction.1) as i32;
    let base_rate = input.base_rate.max(0);
    let added_rate = input.added_rate.max(0);
    let career = input.career.max(0);
    let skill_rate = base_rate.saturating_add(added_rate);
    let base_numerator = post_defense_fraction.0 * base_rate as i128 * career as i128;
    let added_numerator = post_defense_fraction.0 * added_rate as i128 * 1000_i128;
    let skill_fraction = (
        base_numerator + added_numerator,
        post_defense_fraction.1 * 1_000_000_i128,
    );
    let base = (skill_fraction.0 / skill_fraction.1) as i32;
    let multipliers = DamageMultipliers {
        regular: input.regular,
        attack_local: input.attack_local,
        might: input.might,
        action: input.action,
        genesis: input.genesis,
        final_rate: input.final_rate,
    };
    let chained = multipliers
        .values()
        .into_iter()
        .fold(skill_fraction, |(amount, divisor), multiplier| {
            (amount * multiplier.max(0) as i128, divisor * 1000)
        });
    let result = (chained.0 / chained.1) as i32;
    let poison_multiplier = input.poison_multiplier.map(|multiplier| {
        let multiplier = multiplier.max(0);
        let numerator = result as i128 * multiplier as i128;
        let denominator = 1_000;
        PoisonMultiplierTrace {
            input: result,
            multiplier,
            numerator,
            denominator,
            output: (numerator / denominator) as i32,
        }
    });
    let critical_input = poison_multiplier.map_or(result, |trace| trace.output);
    let pre_critical_fraction =
        poison_multiplier.map_or(chained, |trace| (trace.output as i128, 1_i128));
    let critical_fraction = if input.is_crit && critical_multiplier_remainder != 0 {
        multiply_reduced_fraction(
            pre_critical_fraction,
            (
                input.crit_multiplier.max(0) as i128 * 1000
                    + critical_multiplier_remainder.clamp(0, 999) as i128,
                1_000_000,
            ),
        )
    } else if input.is_crit {
        (
            pre_critical_fraction.0 * input.crit_multiplier.max(0) as i128,
            pre_critical_fraction.1 * 1000,
        )
    } else {
        pre_critical_fraction
    };
    let amount = (critical_fraction.0 / critical_fraction.1) as i32;
    DamageTrace {
        defense,
        skill_rate: SkillRateTrace {
            input: post_defense,
            base_rate: input.base_rate,
            added_rate: input.added_rate,
            career: input.career,
            effective_rate: skill_rate,
            base_numerator,
            added_numerator,
            numerator: skill_fraction.0,
            denominator: skill_fraction.1,
            output: base,
        },
        combined_multipliers: CombinedMultiplierTrace {
            input: base,
            multipliers,
            numerator: chained.0,
            denominator: chained.1,
            output: result,
        },
        poison_multiplier,
        critical: CriticalTrace {
            input: critical_input,
            applied: input.is_crit,
            multiplier: input.crit_multiplier,
            numerator: critical_fraction.0,
            denominator: critical_fraction.1,
            output: amount,
        },
        amount,
    }
}
use super::{
    CombinedMultiplierTrace, CriticalTrace, DamageMultipliers, DamageTrace, DefenseTrace,
    PoisonMultiplierTrace, SkillRateTrace,
};
