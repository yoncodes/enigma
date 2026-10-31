use crate::engine::skill::{
    action::SkillRateModifier,
    behavior::{BehaviorOpContext, registry::BehaviorHandler},
    effect::ParsedBehavior,
    rule::output::{BattleCommand, RuleOp},
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [rate] if *rate > 0)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [rate] = behavior.args.as_slice() else {
            return None;
        };
        let origin = super::command_origin(behavior)?;
        let Some((update, counter)) = crate::engine::skill::buff_act::foundation_counter::consume(
            context.managers,
            context.source_uid,
            origin,
        )
        .ok()?
        else {
            return Some(Vec::new());
        };
        let rate = crate::engine::damage::scale_permille(counter, *rate);
        if rate > 0 {
            context.modifiers.rates.push(SkillRateModifier::fixed(
                0,
                behavior.spec.key.opcode,
                rate,
                true,
            ));
        }
        Some(vec![
            RuleOp::Command(BattleCommand::Buff(update.command)),
            RuleOp::BuffActInfoMarker(update.marker),
        ])
    }
}

#[cfg(test)]
mod tests;
