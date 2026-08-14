use crate::engine::entity::attr::AttrId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRateTerm {
    pub opcode: i32,
    pub rate: i32,
    pub career_scaled: bool,
    pub composition: DamageRateComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageRateComposition {
    Additive,
    RetributionLane,
    ProducerMultiplier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackPlan {
    pub source_uid: i64,
    pub target_uid: i64,
    pub skill_id: i32,
    pub rate: i32,
    pub rate_terms: Vec<DamageRateTerm>,
    pub attack_attributes: Vec<(AttrId, i32)>,
    pub career_ratio_bonus: i32,
    pub attack_career: Option<i32>,
    pub additional_attack_career: Option<i32>,
    /// Thousandths of one critical-damage permille point.
    pub critical_multiplier_remainder: i32,
    pub is_conduit: bool,
    pub is_crit: bool,
    pub assassinate: bool,
    pub main_target: bool,
    pub extra_skill_kind: i32,
    pub additional_enabled: bool,
    pub additional_is_crit: Option<bool>,
}
