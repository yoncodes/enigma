use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::action::SkillRequest,
};

#[test]
fn after_skill_keeps_the_configured_consume_then_grant_order() {
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 2,
        buff_id: 100,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(1024, "MonitorContinueChannel"),
        ),
        act_type: "MonitorContinueChannel".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![200, 0, 0, 300],
        raw: String::new(),
    };

    assert!(matches!(
        after_skill(&subscriber).as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                selector: BuffSelector::IdOrType(200),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 300,
                ..
            })))
        ]
    ));
}

#[test]
fn reward_uid_reservation_uses_the_configured_output_buff() {
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 2,
        buff_id: 100,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(1024, "MonitorContinueChannel"),
        ),
        act_type: "MonitorContinueChannel".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![200, 0, 0, 300],
        raw: String::new(),
    };

    assert!(matches!(
        reward_uid_reservation(&subscriber),
        Some(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::ReserveGrantUid(BuffGrantUidReservation {
                target_uid: 10,
                buff_id: 300,
                ..
            })
        )))
    ));
}

#[test]
fn channel_cast_owns_only_the_linked_skill_frame() {
    crate::test_support::init_config();
    let invocation = SkillInvocation::from(SkillRequest {
        source_uid: 10,
        skill_id: 20,
    });
    let command = RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveGrantUid(
        BuffGrantUidReservation {
            origin: super::super::configured_command_origin(
                1024,
                super::super::registry::BuffActKind::MonitorContinueChannel,
            )
            .unwrap(),
            target_uid: 10,
            buff_id: 30,
        },
    )));

    let scoped = scope_rule_ops(vec![command, RuleOp::Skill(invocation)]);

    assert_eq!(
        scoped[0].scope,
        super::super::BuffActFrameScope::CausingFrame
    );
    assert_eq!(
        scoped[1].scope,
        super::super::BuffActFrameScope::SubscriberFrame
    );
    assert_eq!(scoped[1].source, super::super::BuffActFrameSource::Owner);
}
