use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector, BuffSetState},
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill_id, 1, target_code, initial_count]
        if *skill_id > 0 && *target_code > 0 && *initial_count > 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CountContinueChannel) {
        return None;
    }
    let [skill_id, 1, target_code, _] = subscriber.args.as_slice() else {
        return None;
    };
    let BattleEvent::BuffStateChanged(change) = event else {
        return Some(Vec::new());
    };
    if change.target_uid != subscriber.owner_uid
        || change.buff_uid != subscriber.buff_uid
        || change.buff_id != subscriber.buff_id
        || change.before_ex_info <= 0
        || change.after_ex_info != 0
    {
        return Some(Vec::new());
    }

    let origin = super::command_origin(subscriber)?;
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id: *skill_id,
    });
    invocation.target = SkillTarget::LogicRule(*target_code);
    invocation.mode = SkillExecutionMode::Active;

    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: Some(0),
                params: None,
                act_info: None,
            },
        ))),
        RuleOp::Skill(invocation),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }))),
    ])
}

#[cfg(test)]
mod tests;
