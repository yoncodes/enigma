use crate::engine::skill::{
    behavior::{
        AttackModifierContext, BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler,
    },
    effect::ParsedBehavior,
    rule::output::RuleOp,
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
            (BehaviorKind::ChangeAttackCareer, [career]) if (1..=8).contains(career)
        ) || matches!(behavior.spec.kind, BehaviorKind::SetCareerRestraint)
            && behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
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
mod tests {
    use super::*;

    #[test]
    fn career_ratio_fix_keeps_signed_permille_values() {
        assert!(Handler::supports(&ParsedBehavior::new(
            60058,
            "CareerRatioFix",
            vec![300],
        )));
        assert!(Handler::supports(&ParsedBehavior::new(
            60058,
            "CareerRatioFix",
            vec![-600],
        )));
    }

    #[test]
    fn attack_career_accepts_only_real_afflatuses() {
        assert!(Handler::supports(&ParsedBehavior::new(
            100036,
            "SkillChangeAttackCareer",
            vec![1],
        )));
        assert!(!Handler::supports(&ParsedBehavior::new(
            100036,
            "SkillChangeAttackCareer",
            vec![101],
        )));
    }

    #[test]
    fn attack_career_is_written_to_the_skill_modifier() {
        let managers = crate::engine::manager::BattleManagers::default();
        let pool = crate::engine::skill::target::TargetPool::default();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();

        assert!(Handler::collect_attack_modifier(
            AttackModifierContext {
                operation: BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: -1,
                    active_skill_id: 20,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                conditions: &[],
            },
            &ParsedBehavior::new(100036, "SkillChangeAttackCareer", vec![1]),
        ));
        assert_eq!(modifiers.attack_career, Some(1));
    }

    #[test]
    fn active_skill_attack_career_uses_the_same_modifier_path() {
        let managers = crate::engine::manager::BattleManagers::default();
        let pool = crate::engine::skill::target::TargetPool::default();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();

        assert_eq!(
            Handler::emit_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: -1,
                    active_skill_id: 20,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &ParsedBehavior::new(100036, "SkillChangeAttackCareer", vec![1]),
            ),
            Some(Vec::new())
        );
        assert_eq!(modifiers.attack_career, Some(1));
    }
}
