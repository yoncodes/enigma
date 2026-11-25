use std::collections::VecDeque;

use crate::engine::{
    event::payload::BattleEvent,
    manager::field::FieldChangeKind,
    runtime::skill::SkillOpTrigger,
    skill::{
        condition::registry::{ADD_MAGIC_CIRCLE_KEY, ReactionTimingPolicy, find_key},
        rule::{DefinitionKey, output::RuleOp},
    },
};

use super::{
    FramePath, QueuedOp, ReactionBatch, SubscriberContext, insert_after_frame,
    queued_belongs_to_frame_subtree,
};

#[derive(Clone)]
pub(super) enum Scope {
    PendingGrant { owner: FramePath },
    Grant(Grant),
    Reward { completion: FramePath },
}

#[derive(Clone)]
pub(super) struct Grant {
    owner: FramePath,
    completion: FramePath,
}

pub(super) fn grant_scope(
    key: DefinitionKey,
    event: &BattleEvent,
    owner: Option<&[usize]>,
) -> Option<Scope> {
    let BattleEvent::FieldChanged(change) = event else {
        return None;
    };
    let owner = owner?;
    (key == ADD_MAGIC_CIRCLE_KEY
        && change.kind == FieldChangeKind::Level
        && change.after_level > change.before_level)
        .then(|| Scope::PendingGrant {
            owner: owner.to_vec(),
        })
}

pub(super) fn start_grant(scope: &mut Option<Scope>, completion: &[usize]) {
    if let Some(Scope::PendingGrant { owner }) = scope {
        *scope = Some(Scope::Grant(Grant {
            owner: std::mem::take(owner),
            completion: completion.to_vec(),
        }));
    }
}

pub(super) fn command_grant<'a>(
    trigger: &SkillOpTrigger,
    subscriber: Option<&'a SubscriberContext>,
) -> Option<&'a Grant> {
    match (trigger, subscriber?.field_level.as_ref()?) {
        (SkillOpTrigger::Event(BattleEvent::FieldChanged(_)), Scope::Grant(grant)) => Some(grant),
        _ => None,
    }
}

pub(super) fn bind_reactions(
    grant: Option<&Grant>,
    action_path: Option<&[usize]>,
    reactions: &mut ReactionBatch,
) {
    let Some(grant) = grant else {
        return;
    };

    let previous_count = reactions.after_skill.len();
    for mut reaction in std::mem::take(&mut reactions.after_publish) {
        let follows_grant = matches!((&reaction.op, &reaction.trigger),
            (
                RuleOp::Skill(invocation),
                SkillOpTrigger::Event(
                    BattleEvent::BuffAdded(gain) | BattleEvent::BuffChanged(gain),
                ),
            ) if gain.added_type_total.is_some()
                && invocation
                    .condition_key
                    .and_then(|key| find_key(key.opcode, key.type_name))
                    .is_some_and(|rule| {
                        rule.reaction_timing == ReactionTimingPolicy::AfterFieldLevelSkill
                    })
        );
        if follows_grant && let Some(subscriber) = &mut reaction.subscriber {
            subscriber.field_level = Some(Scope::Reward {
                completion: grant.completion.clone(),
            });
            reaction.parent_path = Some(grant.owner.clone());
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

pub(super) fn schedule_after_skill(
    queue: &mut VecDeque<QueuedOp>,
    frame_path: &[usize],
    grant: Option<&Grant>,
    after_skill: Vec<QueuedOp>,
) {
    let Some(grant) = grant else {
        insert_after_frame(queue, frame_path, after_skill);
        return;
    };
    let (rewards, other): (Vec<_>, Vec<_>) = after_skill
        .into_iter()
        .partition(|reaction| completion_anchor(reaction) == Some(&grant.completion));
    if !rewards.is_empty() {
        let index = queue
            .iter()
            .rposition(|queued| {
                queued_belongs_to_frame_subtree(queued, &grant.completion)
                    || command_grant(&queued.trigger, queued.subscriber.as_ref())
                        .is_some_and(|source| source.completion == grant.completion)
                    || completion_anchor(queued) == Some(&grant.completion)
            })
            .map_or(0, |index| index + 1);
        for (offset, reward) in rewards.into_iter().enumerate() {
            queue.insert(index + offset, reward);
        }
    }
    insert_after_frame(queue, frame_path, other);
}

pub(super) fn completion_anchor(after: &QueuedOp) -> Option<&FramePath> {
    match &after.subscriber.as_ref()?.field_level {
        Some(Scope::Reward { completion }) => Some(completion),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
