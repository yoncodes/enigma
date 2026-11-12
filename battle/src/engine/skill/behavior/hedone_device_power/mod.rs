use crate::engine::{
    event::kind::EventKind,
    manager::BattleManagers,
    mechanic::hedone_device_power::HedoneDevicePowerCommand,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillPhase,
        behavior::{
            BehaviorOpContext,
            registry::{self, BehaviorHandler},
        },
        effect::{ParsedBehavior, SkillEffectCatalog},
        rule::{
            DefinitionKey, RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool, TargetResolver, resolve::is_fixed_ally},
    },
};

const KEY: DefinitionKey = DefinitionKey::new(60334, "HedoneAddDevicePower");
const CARD_TRIGGER: DefinitionKey = DefinitionKey::new(210, "None");

pub struct Handler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CardReleaseError {
    pub skill_id: i32,
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        power_id(behavior).is_some()
    }

    fn references(_behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences::default()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Some(vec![RuleOp::Command(BattleCommand::HedoneDevicePower(
            command(behavior, context.target_uid)?,
        ))])
    }
}

fn power_id(behavior: &ParsedBehavior) -> Option<i32> {
    let ([power_id], [raw]) = (behavior.args.as_slice(), behavior.raw_args.as_slice()) else {
        return None;
    };
    (behavior.raw_arity == 1 && *power_id > 0 && raw.parse::<i32>().ok() == Some(*power_id))
        .then_some(*power_id)
}

fn command(behavior: &ParsedBehavior, owner_uid: i64) -> Option<HedoneDevicePowerCommand> {
    let origin = super::command_origin(behavior)?;
    (origin.key == KEY).then_some(HedoneDevicePowerCommand {
        origin,
        owner_uid,
        power_id: power_id(behavior)?,
    })
}

pub(crate) fn card_release(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    source_uid: i64,
    skill_id: i32,
) -> Result<Option<HedoneDevicePowerCommand>, CardReleaseError> {
    let mut slots = catalog
        .get(skill_id)
        .into_iter()
        .flat_map(|effect| &effect.slots)
        .filter(|slot| {
            registry::find(&slot.behavior).is_some_and(|definition| definition.key == KEY)
        });
    let Some(slot) = slots.next() else {
        return Ok(None);
    };
    let invalid = CardReleaseError { skill_id };
    let subscriptions = slot.compiled_subscriptions().map_err(|_| invalid)?;
    let [trigger] = subscriptions.as_slice() else {
        return Err(invalid);
    };
    if slots.next().is_some()
        || trigger.definition != CARD_TRIGGER
        || trigger.event != EventKind::SkillAction
        || trigger.phase != Some(SkillPhase::AfterHit)
        || !matches!(slot.conditions.as_slice(), [condition] if condition.raw_args.is_empty())
        || slot.target_from_condition
        || slot.limit != 0
        || slot.round_limit != 0
        || !is_fixed_ally(slot.target.code)
    {
        return Err(invalid);
    }
    let targets = TargetResolver::resolve_with_managers_and_context(
        &slot.target,
        skill_id,
        source_uid,
        pool,
        &mut RoundDeterminism::default(),
        Some(managers),
        TargetContext::default(),
    );
    let [owner_uid] = targets.as_slice() else {
        return Err(invalid);
    };
    command(&slot.behavior, *owner_uid).map(Some).ok_or(invalid)
}

#[cfg(test)]
mod tests;
