use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{buff::BuffCommand, ex_point::ExPointKind},
    runtime::{
        record::{FrameOwner, FrameTrigger, SemanticFrame, frame_at, round_end_owner_scope},
        skill::SkillOpTrigger,
    },
    skill::rule::{
        DefinitionKey, RuleDomain,
        output::{BattleCommand, RuleOp},
    },
};

pub(super) fn storage_scope<'a>(
    frames: &[SemanticFrame],
    parent: Option<&'a [usize]>,
    trigger: &SkillOpTrigger,
    command: &RuleOp,
) -> Option<&'a [usize]> {
    let RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(storage))) = command
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

    let path = parent.filter(|path| !path.is_empty())?;
    let skill = frame_at(frames, path);
    if !matches!(skill.owner, FrameOwner::Skill { source_uid, .. } if source_uid == storage.target_uid)
        || !matches!(
            skill.trigger,
            FrameTrigger::Event(BattleEvent::Kind(EventKind::RoundEnd))
        )
    {
        return None;
    }
    round_end_owner_scope(frames, path)
}

#[cfg(test)]
mod tests;
