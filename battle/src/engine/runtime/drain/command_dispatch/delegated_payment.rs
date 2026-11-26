use crate::engine::{
    manager::ex_point::{ExPointChange, ExPointChanges, ExPointCommand, ExPointKind},
    skill::action::{SkillExecutionMode, SkillRequest},
};

use super::*;

pub(super) struct Payment {
    action: SkillRequest,
    cost: ExPointChange,
    refund_cap: i32,
}

impl Payment {
    pub(super) fn from_op(op: &RuleOp) -> Option<Self> {
        let RuleOp::PayDelegatedUltimate {
            action,
            cost,
            refund_cap,
        } = *op
        else {
            return None;
        };
        Some(Self {
            action,
            cost,
            refund_cap,
        })
    }

    pub(super) fn settle(
        self,
        outcome: &RuleOutcome,
        queue: &mut VecDeque<QueuedOp>,
        frame_path: &FramePath,
        trigger: &SkillOpTrigger,
        subscriber: Option<&SubscriberContext>,
    ) -> Result<(), DrainError> {
        let RuleOutcome::ExPoint(ExPointChanges::Value { change, .. }) = outcome else {
            return Err(DrainError::MissingDelegatedPaymentCast);
        };
        let Some(RuleOp::Skill(cast)) = queue.front_mut().map(|queued| &mut queued.op) else {
            return Err(DrainError::MissingDelegatedPaymentCast);
        };
        if cast.plan != self.action
            || cast.mode != SkillExecutionMode::DirectBig
            || change.target_uid != self.action.source_uid
        {
            return Err(DrainError::MissingDelegatedPaymentCast);
        }
        let paid = change.applied_delta.saturating_neg().max(0);
        let refund = paid.min(self.refund_cap);
        if change.kind == ExPointKind::Common {
            cast.additional_moxie = paid - refund;
            cast.moxie_spent = paid;
        }
        if refund > 0 {
            queue.insert(
                1,
                QueuedOp {
                    op: RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                        ExPointChange {
                            delta: refund,
                            ..self.cost
                        },
                    ))),
                    trigger: trigger.clone(),
                    skill_execution: None,
                    frame_path: Some(frame_path.clone()),
                    parent_path: None,
                    frame_group: None,
                    independent_parent_group: None,
                    frame_owner: None,
                    subscriber: subscriber.cloned(),
                    caster_frame: None,
                },
            );
        }
        Ok(())
    }
}
