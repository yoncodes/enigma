use crate::engine::{
    event::payload::{BattleEvent, BuffFeatureTriggeredEvent},
    manager::buff::BuffManager,
    skill::{rule::output::RuleOp, subscriber::BuffActSubscriber},
};

use super::registry::BuffActKind;

pub(crate) fn retains_final_charge(buffs: &BuffManager, owner_uid: i64, buff_uid: i64) -> bool {
    buffs.has_buff_act_kind(owner_uid, BuffActKind::LockBulletCountDecr)
        && buffs.buff_has_act(owner_uid, buff_uid, BuffActKind::Bullet)
        && buffs
            .snapshot(owner_uid, buff_uid)
            .is_some_and(|buff| buff.count == Some(1))
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !subscriber.owner_alive {
        return Some(Vec::new());
    }
    match event {
        BattleEvent::SkillAction(action)
            if action.source_uid == subscriber.owner_uid && action.is_attack =>
        {
            Some(vec![RuleOp::Publish(BattleEvent::BuffFeatureTriggered(
                BuffFeatureTriggeredEvent {
                    owner_uid: subscriber.owner_uid,
                    source_uid: subscriber.source_uid,
                    target_uid: action.target_uid,
                    buff_uid: subscriber.buff_uid,
                    buff_id: subscriber.buff_id,
                    act_id: subscriber.key.definition.opcode,
                },
            ))])
        }
        _ => Some(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
