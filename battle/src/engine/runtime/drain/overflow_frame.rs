use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{buff::BuffCommand, ex_point::ExPointKind},
    runtime::{
        record::{
            FrameOwner, FrameTrigger, SemanticFrame, active_skill_scope_path, frame_at,
            round_end_owner_scope,
        },
        skill::SkillOpTrigger,
    },
    skill::action::SkillPhase,
    skill::rule::{
        DefinitionKey, RuleDomain,
        output::{BattleCommand, RuleOp},
    },
};

use super::ReactionBatch;

pub(super) fn storage_scope<'a>(
    frames: &[SemanticFrame],
    parent: Option<&'a [usize]>,
    trigger: &SkillOpTrigger,
    command: &RuleOp,
) -> Option<&'a [usize]> {
    let owner = self_storage(command, trigger)?;
    let path = parent.filter(|path| !path.is_empty())?;
    let skill = frame_at(frames, path);
    if !matches!(skill.owner, FrameOwner::Skill { source_uid, .. } if source_uid == owner) {
        return None;
    }
    match &skill.trigger {
        FrameTrigger::Event(BattleEvent::Kind(EventKind::RoundEnd)) => {
            round_end_owner_scope(frames, path)
        }
        FrameTrigger::Event(event) if critical_owner(event) == Some(owner) => {
            let parent = &path[..path.len() - 1];
            active_skill_scope_path(frames, path)
                .filter(|active| active == parent)
                .map(|_| parent)
        }
        _ => None,
    }
}

pub(super) fn defer_hit_storage(
    frame_path: &[usize],
    action_scope: Option<&[usize]>,
    trigger: &SkillOpTrigger,
    reactions: &mut ReactionBatch,
) {
    let SkillOpTrigger::Event(event) = trigger else {
        return;
    };
    let Some(owner) = critical_owner(event) else {
        return;
    };
    if action_scope.is_none_or(|path| frame_path.len() != path.len() + 1) {
        return;
    }
    let (deferred, immediate): (Vec<_>, _) = std::mem::take(&mut reactions.after_publish)
        .into_iter()
        .partition(|op| self_storage(&op.op, &op.trigger) == Some(owner));
    reactions.after_publish = immediate;
    if deferred.is_empty() {
        return;
    }
    reactions.after_skill.extend(deferred);
    reactions.assign_subscriber_death_groups(action_scope);
}

fn critical_owner(event: &BattleEvent) -> Option<i64> {
    match event {
        BattleEvent::SkillAction(action)
            if action.phase == SkillPhase::AfterDamage && action.crit_count > 0 =>
        {
            Some(action.source_uid)
        }
        _ => None,
    }
}

fn self_storage(command: &RuleOp, trigger: &SkillOpTrigger) -> Option<i64> {
    let RuleOp::Command(BattleCommand::Buff(BuffCommand::BankExPointOverflow(storage))) = command
    else {
        return None;
    };
    let SkillOpTrigger::Event(BattleEvent::ExPointOverflow(change)) = trigger else {
        return None;
    };
    if storage.origin.domain != RuleDomain::BuffAct
        || storage.origin.key != DefinitionKey::new(806, "ExPointOverflowBank")
        || change.origin.domain != RuleDomain::Behavior
        || change.origin.key != DefinitionKey::new(20002, "AddExPoint")
        || change.source_uid != storage.target_uid
        || change.target_uid != storage.target_uid
        || change.applied_delta != 0
        || change.overflow <= 0
        || !matches!(change.kind, ExPointKind::Common | ExPointKind::Faith)
    {
        return None;
    }

    Some(storage.target_uid)
}

#[cfg(test)]
mod tests;
