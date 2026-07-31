use sonettobuf::CardInfo;

use crate::engine::damage::DamageRateComposition;
use crate::engine::entity::attr::AttrId;
use crate::engine::skill::rule::{CommandOrigin, DefinitionKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillPhase {
    Immediate,
    Damage,
    AdditionalDamage,
    AfterDamage,
    HitPassives,
    AfterHit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillRequest {
    pub source_uid: i64,
    pub skill_id: i32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillExecutionMode {
    #[default]
    Nested,
    Active,
    DirectBig,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillStart {
    #[default]
    Immediate,
    AfterCurrentAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillLifecycle {
    DirectUltimateBodyCompleted { source_uid: i64 },
    EmitterAttackStarted(crate::engine::manager::emitter::EmitterAttack),
    EmitterSkillEnded { source_uid: i64 },
    PhaseCompleted(SkillActionEvent),
    ActionCompleted(ActionEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActionEvent {
    pub source_uid: i64,
    pub skill_id: i32,
    pub target_uid: i64,
    pub target_uids: Vec<i64>,
    pub attacked_target_uids: Vec<i64>,
    pub phase: SkillPhase,
    pub skill_slot: i32,
    pub is_attack: bool,
    pub rank: i32,
    pub skill_type: i32,
    pub effect_tag: i32,
    pub assassinate: bool,
    pub damage_amount: i32,
    pub kill_count: i32,
    pub crit_count: i32,
    pub additional_moxie: i32,
    pub extra_skill_kind: i32,
    pub mode: SkillExecutionMode,
    pub teammate_injury_count: i32,
    pub teammate_injury_count_not_reset: i32,
    pub team_injury_count_round: i32,
    pub card_enchants: Vec<i32>,
    pub buff_additions: Vec<(i32, i32)>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ActionEvent {
    pub source_uid: i64,
    pub skill_id: i32,
    pub target_uid: i64,
    pub target_uids: Vec<i64>,
    pub skill_slot: i32,
    pub is_attack: bool,
    pub rank: i32,
    pub skill_type: i32,
    pub effect_tag: i32,
    pub additional_moxie: i32,
    pub extra_skill_kind: i32,
    pub assassinate: bool,
    pub damage_amount: i32,
    pub kill_count: i32,
    pub crit_count: i32,
    pub teammate_injury_count: i32,
    pub teammate_injury_count_not_reset: i32,
    pub team_injury_count_round: i32,
    pub card_enchants: Vec<i32>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillTarget {
    Inherited,
    #[default]
    Configured,
    LogicRule(i32),
    Explicit(i64),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillTrigger {
    #[default]
    None,
    Inherited,
    Explicit(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInvocation {
    pub plan: SkillRequest,
    pub card_index: i32,
    pub additional_moxie: i32,
    pub condition_key: Option<DefinitionKey>,
    pub condition_slot: Option<usize>,
    pub phase: Option<SkillPhase>,
    pub target: SkillTarget,
    pub extra_skill_kind: Option<crate::engine::skill::condition::extra::ExtraSkillKind>,
    pub rate_modifier: Option<SkillRateModifier>,
    pub trigger: SkillTrigger,
    pub mode: SkillExecutionMode,
    pub start: SkillStart,
    pub card_enchants: Vec<i32>,
    pub recorded_skill: Option<SkillRequest>,
}

impl From<SkillRequest> for SkillInvocation {
    fn from(plan: SkillRequest) -> Self {
        Self {
            plan,
            card_index: 0,
            additional_moxie: 0,
            condition_key: None,
            condition_slot: None,
            phase: None,
            target: SkillTarget::Configured,
            extra_skill_kind: None,
            rate_modifier: None,
            trigger: SkillTrigger::None,
            mode: SkillExecutionMode::Nested,
            start: SkillStart::Immediate,
            card_enchants: Vec::new(),
            recorded_skill: None,
        }
    }
}

#[cfg(test)]
mod invocation_tests {
    use super::*;
    use crate::engine::skill::condition::extra::ExtraSkillKind;

    #[test]
    fn invocation_keeps_runtime_target_and_action_kind() {
        let invocation = SkillInvocation {
            plan: SkillRequest {
                source_uid: 10,
                skill_id: 20,
            },
            card_index: 4,
            additional_moxie: 0,
            condition_key: Some(DefinitionKey::new(30, "TestCondition")),
            condition_slot: Some(2),
            phase: Some(SkillPhase::AfterDamage),
            target: SkillTarget::Explicit(-1),
            extra_skill_kind: Some(ExtraSkillKind::FollowUp),
            rate_modifier: None,
            trigger: SkillTrigger::Explicit(30),
            mode: SkillExecutionMode::Nested,
            start: SkillStart::Immediate,
            card_enchants: Vec::new(),
            recorded_skill: None,
        };

        assert_eq!(invocation.target, SkillTarget::Explicit(-1));
        assert_eq!(
            invocation.condition_key,
            Some(DefinitionKey::new(30, "TestCondition"))
        );
        assert_eq!(invocation.card_index, 4);
        assert_eq!(invocation.condition_slot, Some(2));
        assert_eq!(invocation.phase, Some(SkillPhase::AfterDamage));
        assert_eq!(invocation.extra_skill_kind.unwrap().id(), 2);
        assert_eq!(invocation.trigger, SkillTrigger::Explicit(30));
    }

    #[test]
    fn plain_configured_skill_does_not_inherit_runtime_context() {
        let invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 20,
        }
        .into();

        assert_eq!(invocation.target, SkillTarget::Configured);
        assert_eq!(invocation.trigger, SkillTrigger::None);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillRateAmount {
    Fixed(i32),
    GaugeRaw {
        key: crate::engine::manager::gauge::GaugeKey,
        limit: i32,
        factor: i32,
        count: i32,
        divisor: i32,
    },
}

impl SkillRateAmount {
    pub const fn gauge_raw(
        key: crate::engine::manager::gauge::GaugeKey,
        limit: i32,
        factor: i32,
        count: i32,
        divisor: i32,
    ) -> Self {
        Self::GaugeRaw {
            key,
            limit,
            factor,
            count,
            divisor,
        }
    }

    pub const fn fixed_value(self) -> Option<i32> {
        match self {
            Self::Fixed(value) => Some(value),
            Self::GaugeRaw { .. } => None,
        }
    }

    pub fn resolve(self, gauges: &crate::engine::manager::gauge::GaugeManager) -> i32 {
        match self {
            Self::Fixed(value) => value,
            Self::GaugeRaw {
                key,
                limit,
                factor,
                count,
                divisor,
            } if limit >= 0 && factor >= 0 && count >= 0 && divisor > 0 => {
                let raw = i64::from(gauges.raw_value(key).unwrap_or_default().clamp(0, limit));
                (raw * i64::from(factor) * i64::from(count) / i64::from(divisor))
                    .try_into()
                    .unwrap_or(i32::MAX)
            }
            Self::GaugeRaw { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillRateModifier {
    pub target_uid: i64,
    pub opcode: i32,
    pub amount: SkillRateAmount,
    pub career_scaled: bool,
    pub composition: DamageRateComposition,
}

impl SkillRateModifier {
    pub const fn fixed_value(self) -> Option<i32> {
        self.amount.fixed_value()
    }

    pub const fn fixed(target_uid: i64, opcode: i32, delta: i32, career_scaled: bool) -> Self {
        Self::new(
            target_uid,
            opcode,
            SkillRateAmount::Fixed(delta),
            career_scaled,
        )
    }

    pub const fn new(
        target_uid: i64,
        opcode: i32,
        amount: SkillRateAmount,
        career_scaled: bool,
    ) -> Self {
        Self {
            target_uid,
            opcode,
            amount,
            career_scaled,
            composition: DamageRateComposition::Additive,
        }
    }

    pub const fn retribution_lane(opcode: i32, delta: i32) -> Self {
        Self {
            target_uid: 0,
            opcode,
            amount: SkillRateAmount::Fixed(delta),
            career_scaled: true,
            composition: DamageRateComposition::RetributionLane,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalDamageModifier {
    pub origin: CommandOrigin,
    pub buff_id: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillModifiers {
    pub rates: Vec<SkillRateModifier>,
    pub attack_attributes: Vec<(AttrId, i32)>,
    pub redirected_damage_targets: Vec<i64>,
    pub force_critical: bool,
    pub excess_crit_conversion_rate: i32,
    pub career_ratio_bonus: i32,
    pub attack_career: Option<i32>,
    pub additional_damage: Vec<AdditionalDamageModifier>,
    pub consume_team_injury_count_round: Option<DefinitionKey>,
}

impl SkillModifiers {
    pub(crate) fn merge(&mut self, mut other: Self) {
        self.rates.append(&mut other.rates);
        self.attack_attributes.append(&mut other.attack_attributes);
        for target_uid in other.redirected_damage_targets {
            if !self.redirected_damage_targets.contains(&target_uid) {
                self.redirected_damage_targets.push(target_uid);
            }
        }
        self.force_critical |= other.force_critical;
        self.excess_crit_conversion_rate += other.excess_crit_conversion_rate;
        self.career_ratio_bonus += other.career_ratio_bonus;
        self.attack_career = self.attack_career.or(other.attack_career);
        self.additional_damage.append(&mut other.additional_damage);
        self.consume_team_injury_count_round = self
            .consume_team_injury_count_round
            .or(other.consume_team_injury_count_round);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSkill {
    pub plan: SkillRequest,
    pub target_uid: Option<i64>,
    pub card_index: i32,
    pub card: CardInfo,
}

impl From<crate::engine::manager::card::PlayedCard> for PlannedSkill {
    fn from(played: crate::engine::manager::card::PlayedCard) -> Self {
        Self {
            plan: SkillRequest {
                source_uid: played.caster_uid,
                skill_id: played.skill_id,
            },
            target_uid: played.target_uid,
            card_index: played.card_index,
            card: played.card,
        }
    }
}
