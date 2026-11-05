use super::DamageKind;

pub(crate) fn enabled() -> bool {
    crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum DefenseTrace {
    Applied {
        kind: DamageKind,
        attack: i32,
        raw_defense: i32,
        defense_multiplier: i32,
        modified_defense: i32,
        penetration: i32,
        effective_defense: i32,
        minimum: i32,
        minimum_numerator: i128,
        minimum_denominator: i128,
        calculated_output: i32,
        output: i32,
    },
    Skipped {
        input: i32,
    },
}

impl DefenseTrace {
    pub fn output(self) -> i32 {
        match self {
            Self::Applied { output, .. } => output,
            Self::Skipped { input } => input,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SkillRateTrace {
    pub input: i32,
    pub base_rate: i32,
    pub added_rate: i32,
    pub career: i32,
    pub effective_rate: i32,
    pub base_numerator: i128,
    pub added_numerator: i128,
    pub numerator: i128,
    pub denominator: i128,
    pub output: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DamageMultipliers {
    pub regular: i32,
    pub attack_local: i32,
    pub might: i32,
    pub action: i32,
    pub genesis: i32,
    pub final_rate: i32,
}

impl DamageMultipliers {
    pub(super) fn values(self) -> [i32; 6] {
        [
            self.regular,
            self.attack_local,
            self.might,
            self.action,
            self.genesis,
            self.final_rate,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CombinedMultiplierTrace {
    pub input: i32,
    pub multipliers: DamageMultipliers,
    pub numerator: i128,
    pub denominator: i128,
    pub output: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CriticalTrace {
    pub input: i32,
    pub applied: bool,
    pub multiplier: i32,
    pub numerator: i128,
    pub denominator: i128,
    pub output: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PoisonMultiplierTrace {
    pub input: i32,
    pub multiplier: i32,
    pub numerator: i128,
    pub denominator: i128,
    pub output: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageStageTrace {
    Defense(DefenseTrace),
    SkillRate(SkillRateTrace),
    CombinedMultipliers(CombinedMultiplierTrace),
    PoisonMultiplier(PoisonMultiplierTrace),
    Critical(CriticalTrace),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DamageTrace {
    pub defense: DefenseTrace,
    pub skill_rate: SkillRateTrace,
    pub combined_multipliers: CombinedMultiplierTrace,
    pub poison_multiplier: Option<PoisonMultiplierTrace>,
    pub critical: CriticalTrace,
    pub amount: i32,
}

#[derive(serde::Serialize)]
pub(super) struct HitTrace<'a> {
    pub skill_id: i32,
    pub source_uid: i64,
    pub target_uid: i64,
    pub fight_version: i32,
    pub formula: super::DamageFormula,
    pub trace: &'a DamageTrace,
}

impl HitTrace<'_> {
    pub fn emit(&self) {
        match serde_json::to_string(self) {
            Ok(json) => eprintln!("damage-json {json}"),
            Err(error) => eprintln!("damage-json error: {error}"),
        }
    }
}

impl DamageTrace {
    pub fn stages(self) -> [Option<DamageStageTrace>; 5] {
        [
            Some(DamageStageTrace::Defense(self.defense)),
            Some(DamageStageTrace::SkillRate(self.skill_rate)),
            Some(DamageStageTrace::CombinedMultipliers(
                self.combined_multipliers,
            )),
            self.poison_multiplier
                .map(DamageStageTrace::PoisonMultiplier),
            Some(DamageStageTrace::Critical(self.critical)),
        ]
    }
}
