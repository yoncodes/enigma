use crate::engine::manager::entity::{EntityCommand, EntityOperation};
use crate::engine::skill::{
    behavior::{
        AttackModifierContext, BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler,
    },
    effect::ParsedBehavior,
    rule::output::{BattleCommand, RuleOp},
};

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::CareerRatioFix, [bonus]) if *bonus != 0
        ) || matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::ChangeCareer, [career]) if (1..=6).contains(career)
        ) || matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::ChangeAttackCareer, [career]) if (1..=8).contains(career)
        ) || matches!(behavior.spec.kind, BehaviorKind::SetCareerRestraint)
            && behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if let (BehaviorKind::ChangeCareer, [career]) =
            (behavior.spec.kind, behavior.args.as_slice())
        {
            return (1..=6).contains(career).then(|| {
                vec![RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    operation: EntityOperation::ChangeCareer { career: *career },
                }))]
            });
        }
        apply(context.modifiers, behavior).then(Vec::new)
    }

    fn collect_attack_modifier(
        context: AttackModifierContext<'_>,
        behavior: &ParsedBehavior,
    ) -> bool {
        apply(context.operation.modifiers, behavior)
    }
}

fn apply(
    modifiers: &mut crate::engine::skill::action::SkillModifiers,
    behavior: &ParsedBehavior,
) -> bool {
    if !Handler::supports(behavior) {
        return false;
    }
    match behavior.spec.kind {
        BehaviorKind::CareerRatioFix => {
            modifiers.career_ratio_bonus += behavior.args[0];
        }
        BehaviorKind::ChangeAttackCareer => {
            modifiers.attack_career = Some(behavior.args[0]);
        }
        BehaviorKind::SetCareerRestraint => modifiers.force_career_restraint = true,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests;
