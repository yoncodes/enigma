use crate::engine::{
    event::payload::BattleEvent,
    manager::field::FieldChangeKind,
    runtime::skill::SkillOpTrigger,
    skill::{
        condition::registry::{ReactionTimingPolicy, find_key},
        rule::output::RuleOp,
    },
};

use super::{FramePath, QueuedOp, ReactionBatch};

pub(super) fn bind_reactions(
    frame_path: &[usize],
    action_path: Option<&[usize]>,
    trigger: &SkillOpTrigger,
    reactions: &mut ReactionBatch,
) {
    let SkillOpTrigger::Event(BattleEvent::FieldChanged(change)) = trigger else {
        return;
    };
    let Some((_, parent)) = frame_path.split_last() else {
        return;
    };
    if change.kind != FieldChangeKind::Level
        || change.after_level <= change.before_level
        || parent.is_empty()
    {
        return;
    }

    let previous_count = reactions.after_skill.len();
    for mut reaction in std::mem::take(&mut reactions.after_publish) {
        let invocation = match (&mut reaction.op, &reaction.trigger) {
            (
                RuleOp::Skill(invocation),
                SkillOpTrigger::Event(
                    BattleEvent::BuffAdded(gain) | BattleEvent::BuffChanged(gain),
                ),
            ) if gain.added_type_total.is_some()
                && reaction.parent_path.as_deref() == Some(frame_path)
                && invocation
                    .condition_key
                    .and_then(|key| find_key(key.opcode, key.type_name))
                    .is_some_and(|rule| {
                        rule.reaction_timing == ReactionTimingPolicy::AfterFieldLevelSkill
                    }) =>
            {
                Some(invocation)
            }
            _ => None,
        };
        if let Some(invocation) = invocation {
            invocation.field_level_settlement = true;
            reaction.frame_group = None;
            reactions.after_skill.push(reaction);
        } else {
            reactions.after_publish.push(reaction);
        }
    }
    if reactions.after_skill.len() != previous_count {
        reactions.assign_subscriber_death_groups(action_path);
    }
}

pub(super) fn completion_anchor(after: &QueuedOp) -> Option<FramePath> {
    match &after.op {
        RuleOp::Skill(invocation) if invocation.field_level_settlement => after.parent_path.clone(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
