use crate::engine::{
    event::payload::{BattleEvent, BuffFeatureTriggeredEvent},
    skill::{rule::output::RuleOp, subscriber::BuffActSubscriber},
};

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
