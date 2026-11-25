use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    skill::{
        action::SkillTrigger,
        condition::ParsedConditionKind,
        effect::{SkillEffectCatalog, SkillEffectSlot},
        target::request::SOURCE_TARGET_CODE,
    },
};

use super::{
    BehaviorHandler, BehaviorOpContext, ParsedBehavior, RuleOp, RuleReferences,
    direct_no_action_skill,
};

pub(in crate::engine::skill::behavior) struct Handler;

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        super::references(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        direct_no_action_skill(context, behavior)
    }

    fn emit_runtime_ops(
        context: BehaviorOpContext<'_>,
        slot: &SkillEffectSlot,
        _: &SkillEffectCatalog,
    ) -> Option<Vec<RuleOp>> {
        let assassination_reward = matches!(context.event, Some(BattleEvent::AllyAction(action)) if action.assassinate)
            && context.target_uid == context.source_uid
            && !slot.target_from_condition
            && slot.target.code == SOURCE_TARGET_CODE
            && slot.target.raw.is_empty()
            && matches!(
                slot.conditions.as_slice(),
                [condition] if condition.kind == ParsedConditionKind::Assassinate
                    && condition.raw_args.is_empty()
                    && slot.compiled_subscriptions().is_ok_and(|subscriptions|
                        subscriptions.iter().any(|route|
                            route.event == EventKind::AllyAction
                                && route.definition.matches(condition.opcode, &condition.type_name)))
            );
        let mut ops = Self::emit_ops(context, &slot.behavior)?;
        if assassination_reward && let [RuleOp::Skill(child)] = ops.as_mut_slice() {
            child.trigger = SkillTrigger::AssassinationReward;
        }
        Some(ops)
    }
}
