use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::rule::DefinitionKey,
};

fn played(uid: i64, skill_id: i32) -> PlayedCard {
    PlayedCard {
        card: CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            ..Default::default()
        },
        caster_uid: uid,
        card_index: 1,
        hand_index: None,
        skill_id,
        uses_action_point: false,
        rank_change_pending: false,
        rewritten: false,
        target_uid: None,
        recorded_skill: None,
    }
}

#[test]
fn records_matching_basic_cards_in_play_order() {
    let cards = [played(1, 10), played(2, 20), played(3, 30), played(1, 10)];

    assert_eq!(
        match_recorded_cards(&cards, &[10, 30, 10], 2)
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![10, 30]
    );
}

#[test]
fn accepts_only_the_proven_argument_shape() {
    assert!(supports(&[100, 2, 1]));
    assert!(!supports(&[100, 2, 0]));
    assert!(!supports(&[100, 0, 1]));
}

#[test]
fn releases_once_after_the_action_queue_is_committed() {
    let mut managers = BattleManagers::default();
    managers.card.record_cast_channel(
        20,
        vec![
            CardInfo {
                uid: Some(11),
                skill_id: Some(101),
                ..Default::default()
            },
            CardInfo {
                uid: Some(12),
                skill_id: Some(102),
                ..Default::default()
            },
        ],
    );
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::ActionQueueCommitted,
            DefinitionKey::new(923, "AddCardCastChannel"),
        ),
        act_type: "AddCardCastChannel".to_owned(),
        effect_time: 1062,
        effect_condition: 0,
        args: vec![200, 2, 1],
        raw: "923#200#2#1".to_owned(),
    };
    let event = BattleEvent::ActionQueueCommitted {
        team: 1,
        emitter_uid: 99998,
        cards: vec![CardInfo::default(); 3],
        action_count: 3,
    };

    let ops = rule_ops(&managers, &subscriber, &event).unwrap();

    assert!(matches!(
        ops.first(),
        Some(RuleOp::Skill(invocation))
            if invocation.plan == (SkillRequest { source_uid: 10, skill_id: 200 })
    ));
    for (offset, op) in ops[1..3].iter().enumerate() {
        assert!(matches!(
            op,
            RuleOp::Command(BattleCommand::Card(CardCommand::QueueUseCard(queue)))
                if queue.card_index == 4 + offset as i32
                    && queue.source_skill_id == 200
                    && queue.action.as_ref().is_some_and(|action|
                        action.mode == SkillExecutionMode::Active)
        ));
    }
    assert!(matches!(
        ops.last(),
        Some(RuleOp::Command(BattleCommand::Card(
            CardCommand::ResolveCastChannel { buff_uid: 20, .. }
        )))
    ));

    managers.card.resolve_cast_channel(20);
    assert_eq!(rule_ops(&managers, &subscriber, &event), Some(Vec::new()));
}
