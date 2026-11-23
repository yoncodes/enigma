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
