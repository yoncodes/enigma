use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::BattleManagers,
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    skills(args).is_some()
}

pub(super) fn skills(args: &[i32]) -> Option<&[i32; 4]> {
    let skills: &[i32; 4] = args.try_into().ok()?;
    skills.iter().all(|&skill| skill > 0).then_some(skills)
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if event.kind() != EventKind::RoundEnd {
        return Some(Vec::new());
    }
    let level = managers
        .field
        .get(subscriber.team_type)
        .filter(|field| {
            managers
                .catalog()
                .is_electric_field(field.definition.field_id)
        })
        .map_or(0, |field| field.level);
    let &skill_id = skills(&subscriber.args)?.get(usize::try_from(level).ok()?)?;
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    });
    invocation.mode = SkillExecutionMode::Active;
    invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(skill_id));
    Some(vec![RuleOp::Skill(invocation)])
}

#[cfg(test)]
mod tests;
