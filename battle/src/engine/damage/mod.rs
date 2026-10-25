pub mod formula;
pub mod handler;
pub mod heal_settlement;
pub mod modifiers;
mod pipeline;
mod plan;
mod settlement;
mod trace;

pub use formula::{
    DamageFormula, DamageFormulaRules, DeadlyPoisonFormulaInput, attribute_scaled_damage,
    butterfly_damage, deadly_poison_damage, scale_permille, scale_permille_stacks,
};
pub use pipeline::{
    DamageFormulaInput, DamageKind, MinimumDamage, calculate, calculate_with_trace,
    calculate_with_trace_for_version,
};
pub use plan::{AttackPlan, DamageRateComposition, DamageRateTerm};
pub use settlement::DamageSettlement;
pub(crate) use trace::enabled as trace_enabled;
pub use trace::{
    CombinedMultiplierTrace, CriticalTrace, DamageMultipliers, DamageStageTrace, DamageTrace,
    DefenseTrace, PoisonMultiplierTrace, SkillRateTrace,
};

#[cfg(test)]
mod tests;
