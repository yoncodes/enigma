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
    assert!(!scoped[0].group_with_siblings);
    assert_eq!(
        scoped[1].scope,
        super::super::BuffActFrameScope::SubscriberFrame
    );
    assert!(scoped[1].group_with_siblings);
    assert_eq!(scoped[1].source, super::super::BuffActFrameSource::Owner);
}

#[test]
fn final_settlement_casts_for_each_remaining_lock() {
    crate::test_support::init_config();
    let owner_uid = 10;
    let lock = 31260141;
    let cast = 31260171;
    let subscriber = BuffActSubscriber {
        owner_uid,
        source_uid: owner_uid,
        buff_uid: 2,
        buff_id: 31260131,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEndFinalSettlement,
            crate::engine::skill::rule::DefinitionKey::new(1024, "MonitorContinueChannel"),
        ),
        act_type: "MonitorContinueChannel".to_owned(),
        effect_time: 307,
        effect_condition: 0,
        args: vec![lock, cast, 31260151, 31260161],
        raw: String::new(),
    };
    let mut fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(owner_uid),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: super::super::command_origin(&subscriber).unwrap(),
            source_uid: owner_uid,
            target_uid: owner_uid,
            buff_id: lock,
            amount: Some(3),
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(managers.buff.buff_id_or_type_amount(owner_uid, lock), 3);

    let event = BattleEvent::BuffsSettled(Vec::new());
    let ops = rule_ops(
        &managers,
        &TargetPool::from_fight(&fight),
        &subscriber,
        &event,
    )
    .unwrap();

    assert_eq!(ops.len(), 12);
    for batch in ops.chunks_exact(4) {
        assert!(matches!(&batch[1], RuleOp::Skill(invocation) if invocation.plan.skill_id == cast));
        assert!(matches!(
            &batch[2],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid,
                selector: BuffSelector::IdOrType(id),
                amount: 1,
                ..
            }))) if *target_uid == owner_uid && *id == lock
        ));
    }

    let defender = fight.defender.as_mut().unwrap();
    defender.sub_entitys = std::mem::take(&mut defender.entitys);
    let pool = TargetPool::from_fight(&fight);
    assert!(
        rule_ops(&managers, &pool, &subscriber, &event)
            .unwrap()
            .is_empty()
    );
}
