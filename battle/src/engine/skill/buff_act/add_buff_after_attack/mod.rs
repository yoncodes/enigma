use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        action::SkillPhase,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddBuffAfterAttack) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if action.phase != SkillPhase::AfterDamage
        || !action.is_attack
        || action.source_uid != subscriber.owner_uid
    {
        return Some(Vec::new());
    }
    let [buff_id, amount] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::GrantIndependent(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: action.source_uid,
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(*amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, amount] if *buff_id > 0 && *amount > 0)
}

#[cfg(test)]
mod tests;
