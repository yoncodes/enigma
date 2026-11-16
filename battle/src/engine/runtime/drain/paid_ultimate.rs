use super::*;
use crate::engine::{
    event::kind::EventKind, manager::ex_point::ExPointCommand, runtime::record::frame_at,
    skill::action::PaidUltimate,
};

pub(super) fn start(
    queue: &mut VecDeque<QueuedOp>,
    frames: &[SemanticFrame],
    paid: PaidUltimate,
    cast: QueuedOp,
) -> Result<(), DrainError> {
    let path = cast
        .frame_path
        .as_deref()
        .ok_or(DrainError::MissingRoundEndPaymentFrame)?;
    let round_end = BattleEvent::Kind(EventKind::RoundEnd);
    let scope = (1..path.len())
        .rev()
        .map(|length| &path[..length])
        .find(|ancestor| {
            let frame = frame_at(frames, ancestor);
            matches!(frame.owner, FrameOwner::EventRule)
                && matches!(
                    frame.trigger,
                    FrameTrigger::Event(BattleEvent::Kind(EventKind::RoundEnd))
                )
        })
        .ok_or(DrainError::MissingRoundEndPaymentFrame)?;
    let payment = QueuedOp {
        op: RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Spend(paid.cost))),
        trigger: SkillOpTrigger::Event(round_end),
        skill_execution: None,
        frame_path: Some(scope.to_vec()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    };
    prepend(queue, [payment, cast]);
    Ok(())
}
