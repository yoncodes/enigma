use crate::engine::{
    event::kind::EventKind,
    manager::conduit::ConduitPowerChangeKind,
    skill::{
        action::SkillPhase, behavior::registry, effect::SkillEffectSlot, rule::DefinitionKey,
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) fn conduit_power_args(args: &[i32]) -> Option<(i32, i32, ConduitPowerChangeKind)> {
    match args {
        [power_id, delta] if *power_id >= 0 => {
            Some((*power_id, *delta, ConduitPowerChangeKind::Standard))
        }
        [power_id, delta, 1] if *power_id >= 0 => {
            Some((*power_id, *delta, ConduitPowerChangeKind::Interval))
        }
        _ => None,
    }
}

pub(crate) fn fixed_conduit_gain(slot: &SkillEffectSlot) -> Option<(i32, i32)> {
    if registry::find(&slot.behavior)?.key != DefinitionKey::new(60291, "AddDevicePower")
        || slot.target.code != SOURCE_TARGET_CODE
        || slot.target_from_condition
        || slot.limit != 0
        || slot.round_limit != 0
    {
        return None;
    }
    let [condition] = slot.conditions.as_slice() else {
        return None;
    };
    if !condition.raw_args.is_empty() {
        return None;
    }
    let subscriptions = slot.compiled_subscriptions().ok()?;
    let [trigger] = subscriptions.as_slice() else {
        return None;
    };
    if trigger.definition != DefinitionKey::new(210, "None")
        || trigger.event != EventKind::SkillAction
        || trigger.phase != Some(SkillPhase::AfterHit)
    {
        return None;
    }
    let [power, amount] = slot.behavior.raw_args.as_slice() else {
        return None;
    };
    let (power_id, delta, kind) = conduit_power_args(&slot.behavior.args)?;
    (kind == ConduitPowerChangeKind::Standard
        && delta > 0
        && power.parse::<i32>().ok() == Some(power_id)
        && amount.parse::<i32>().ok() == Some(delta))
    .then_some((power_id, delta))
}
