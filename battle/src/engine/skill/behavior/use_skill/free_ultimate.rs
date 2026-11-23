use crate::engine::skill::{
    action::{SkillExecutionMode, SkillInvocation, SkillRequest},
    behavior::BehaviorOpContext,
    rule::output::RuleOp,
};

pub(super) fn cast(context: BehaviorOpContext<'_>) -> Option<Vec<RuleOp>> {
    let caster = context.pool.entity(context.target_uid)?;
    if caster.ex_skill <= 0 {
        return Some(Vec::new());
    }
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: caster.uid,
        skill_id: caster.ex_skill,
    });
    invocation.mode = SkillExecutionMode::Direct;
    Some(vec![RuleOp::Skill(invocation)])
}
