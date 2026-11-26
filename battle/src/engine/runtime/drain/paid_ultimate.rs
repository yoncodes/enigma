use super::*;
use crate::engine::{
    event::kind::EventKind, manager::ex_point::ExPointCommand,
    runtime::record::round_end_owner_scope, skill::action::PaidUltimate,
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
    let scope =
        round_end_owner_scope(frames, path).ok_or(DrainError::MissingRoundEndPaymentFrame)?;
    let RuleOp::Skill(invocation) = &cast.op else {
        return Err(DrainError::Command(RuleExecutionError::UnexpectedSkill));
    };
    let payment = QueuedOp {
        op: RuleOp::DebitUltimate {
            action: invocation.plan,
            cost: ExPointCommand::Spend(paid.cost),
        },
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
