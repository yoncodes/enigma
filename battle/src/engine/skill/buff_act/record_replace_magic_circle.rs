use crate::engine::{
    event::kind::EventKind,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        emitter,
        field::{FieldCommand, FieldOperation},
    },
    skill::{
        buff_act::{
            BuffActFrameOwner, BuffActFrameScope, BuffActFrameSource, BuffActRuleOp,
            registry::RuntimeContext,
        },
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordReplaceMagicCircleError {
    Origin,
    Circle(i32),
    Return(i32),
}

pub fn scoped_rule_ops(
    context: &mut RuntimeContext<'_>,
) -> Result<Vec<BuffActRuleOp>, super::BuffActRuleError> {
    Ok(round_end_rule_ops(context.managers, context.subscriber)?)
}

fn round_end_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
) -> Result<Vec<BuffActRuleOp>, RecordReplaceMagicCircleError> {
    let origin = super::command_origin(subscriber).ok_or(RecordReplaceMagicCircleError::Origin)?;
    let Some(field) = managers.field.get(subscriber.team_type) else {
        return Ok(Vec::new());
    };
    if field.owner_uid != subscriber.owner_uid
        || field.definition.duration <= 0
        || managers.field.was_deployed_this_round(field.team)
    {
        return Ok(Vec::new());
    }

    let circle_id = field.definition.field_id;
    let current = managers
        .catalog()
        .magic_circle(circle_id)
        .ok_or(RecordReplaceMagicCircleError::Circle(circle_id))?;
    let returned = managers
        .catalog()
        .return_magic_circle_level(circle_id)
        .ok_or(RecordReplaceMagicCircleError::Return(circle_id))?;

    let lifecycle = |op| BuffActRuleOp {
        op,
        scope: BuffActFrameScope::SubscriberFrame,
        source: BuffActFrameSource::Owner,
        group_with_siblings: true,
        frame_owner: BuffActFrameOwner::Command,
    };
    if field.definition.duration > 1 {
        return Ok(vec![lifecycle(RuleOp::Command(BattleCommand::Field(
            FieldCommand {
                origin,
                team: field.team,
                operation: FieldOperation::ChangeDuration { delta: -1 },
            },
        )))]);
    }

    let mut operations = current
        .linked_buffs
        .into_iter()
        .filter(|buff_id| managers.buff.has_buff_id(emitter::UID, *buff_id))
        .map(|buff_id| {
            lifecycle(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::RemoveAfterTrigger(BuffRemove {
                    origin,
                    target_uid: emitter::UID,
                    selector: BuffRemoveSelector::ExactId(buff_id),
                }),
            )))
        })
        .collect::<Vec<_>>();
    operations.push(lifecycle(RuleOp::Command(BattleCommand::Field(
        FieldCommand {
            origin,
            team: field.team,
            operation: FieldOperation::Remove,
        },
    ))));
    operations.extend(returned.linked_buffs.into_iter().map(|buff_id| {
        BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: field.owner_uid,
                target_uid: emitter::UID,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))))
    }));
    operations.push(BuffActRuleOp::causing(RuleOp::Command(
        BattleCommand::Field(FieldCommand {
            origin,
            team: field.team,
            operation: FieldOperation::DeployReplacement {
                definition: returned.definition,
                owner_uid: field.owner_uid,
                create_uid: -1,
                level: 0,
            },
        }),
    )));
    Ok(operations)
}

#[cfg(test)]
mod tests;
