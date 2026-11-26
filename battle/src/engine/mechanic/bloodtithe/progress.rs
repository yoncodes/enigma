use crate::engine::{
    manager::{
        BattleManagers,
        fight_progress::{LegacyProgressAdd, LegacyProgressDelta},
        gauge::GaugeKey,
    },
    runtime::executor::RuleExecutionError,
    skill::{
        behavior::BehaviorOpContext,
        condition::ParsedConditionKind,
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gain {
    origin: CommandOrigin,
    key: GaugeKey,
    rule_skill: i32,
    per_point: i32,
}

pub(crate) fn parse(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [direction, threshold, limit] = args else {
        return None;
    };
    (direction == "0" && threshold == "1" && limit == "999")
        .then_some(ParsedConditionKind::BloodtitheProgress)
}

pub(crate) fn matches(slot: &SkillEffectSlot) -> bool {
    matches!(
        slot.conditions.as_slice(),
        [condition] if condition.kind == ParsedConditionKind::BloodtitheProgress
    )
}

pub(crate) fn rule_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let gain = Gain {
        origin: crate::engine::skill::behavior::command_origin(behavior)?,
        key: super::rule::key(context.source_team),
        rule_skill: context.active_skill_id,
        per_point: behavior.arg(1)?,
    };
    if pending(context.managers, gain) == 0 {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(
        BattleCommand::LegacyProgressBloodtithe(gain),
    )])
}

fn pending(managers: &BattleManagers, gain: Gain) -> i32 {
    managers
        .gauge
        .unclaimed_positive_gains(gain.key, gain.rule_skill)
}

pub(crate) fn settle(
    managers: &mut BattleManagers,
    gain: Gain,
) -> Result<Option<LegacyProgressDelta>, RuleExecutionError> {
    let points = pending(managers, gain);
    if points == 0 {
        return Ok(None);
    }
    let change = managers.fight_progress.add_legacy(LegacyProgressAdd {
        origin: gain.origin,
        amount: points.saturating_mul(gain.per_point),
    })?;
    managers
        .gauge
        .claim_positive_gains(gain.key, gain.rule_skill);
    Ok(change)
}

#[cfg(test)]
mod tests;
