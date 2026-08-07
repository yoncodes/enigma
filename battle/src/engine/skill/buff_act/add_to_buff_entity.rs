use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddToBuffEntity)
        || event.kind() != EventKind::RoundEndAfterSettlement
    {
        return None;
    }
    let [buff_id, layer] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Grant(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: if subscriber.source_uid != 0 {
                subscriber.source_uid
            } else {
                subscriber.owner_uid
            },
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(*layer),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, layer] if *buff_id > 0 && *layer > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::subscription::SubscriptionKey,
        manager::buff::BuffGrant,
        skill::{rule::DefinitionKey, subscriber::BuffActSubscriber},
    };

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: -3,
            source_uid: -3,
            buff_uid: 41,
            buff_id: 117300301,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundEndAfterSettlement,
                DefinitionKey::new(745, "AddToBuffEntity"),
            ),
            act_type: "AddToBuffEntity".to_owned(),
            effect_time: 304,
            effect_condition: 0,
            args: vec![4_150_001, 1],
            raw: "745#4150001#1".to_owned(),
        }
    }

    #[test]
    fn round_end_grants_the_configured_buff_to_the_buff_owner() {
        let ops = rule_ops(
            &subscriber(),
            &BattleEvent::Kind(EventKind::RoundEndAfterSettlement),
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    source_uid: -3,
                    target_uid: -3,
                    buff_id: 4_150_001,
                    amount: Some(1),
                    ..
                }
            )))]
        ));
        assert!(rule_ops(&subscriber(), &BattleEvent::Kind(EventKind::RoundEnd)).is_none());
    }
}
