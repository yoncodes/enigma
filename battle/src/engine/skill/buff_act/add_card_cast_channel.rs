use sonettobuf::CardInfo;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        card::{CardCommand, CardQueueUse, CardRecordCastChannel, CastChannelState, PlayedCard},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [release_skill, count, mode]
        if *release_skill > 0 && *count > 0 && *mode == 1)
}

pub fn referenced_skill(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[0])
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let (change, added) = match event {
        BattleEvent::BuffAdded(change) => (change, true),
        BattleEvent::BuffRemoved(change) => (change, false),
        _ => return Vec::new(),
    };
    managers
        .buff
        .definition_features(change.buff_id)
        .into_iter()
        .filter_map(|mut feature| {
            if super::feature_kind(&feature)? != super::registry::BuffActKind::AddCardCastChannel {
                return None;
            }
            let [_, _, count, 1] = feature.values.as_slice() else {
                return None;
            };
            feature.owner_uid = change.target_uid;
            feature.source_uid = change.source_uid;
            feature.buff_uid = change.buff_uid;
            feature.amount = change.after_amount;
            let origin = super::feature_command_origin(&feature)?;
            let command = if added {
                let recorded = super::card_record::recorded_skill_ids(managers, change.target_uid);
                let cards =
                    match_recorded_cards(managers.card.played(), &recorded, *count as usize);
                CardCommand::RecordCastChannel(CardRecordCastChannel {
                    origin,
                    buff_uid: change.buff_uid,
                    cards,
                })
            } else {
                CardCommand::RemoveCastChannel {
                    origin,
                    buff_uid: change.buff_uid,
                }
            };
            Some((feature, RuleOp::Command(BattleCommand::Card(command))))
        })
        .collect()
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::ActionQueueCommitted { cards, .. } = event else {
        return None;
    };
    let [release_skill, _, 1] = subscriber.args.as_slice() else {
        return None;
    };
    let remembered = match managers.card.cast_channel(subscriber.buff_uid)? {
        CastChannelState::Pending(cards) => cards,
        CastChannelState::Resolved => return Some(Vec::new()),
    };
    let origin = super::command_origin(subscriber)?;
    let mut ops = vec![RuleOp::Skill(
        SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id: *release_skill,
        }
        .into(),
    )];
    for (offset, card) in remembered.iter().cloned().enumerate() {
        let card_index = cards.len() as i32 + offset as i32 + 1;
        let mut action: SkillInvocation = SkillRequest {
            source_uid: card.uid?,
            skill_id: card.skill_id?,
        }
        .into();
        action.card_index = card_index;
        action.mode = SkillExecutionMode::Active;
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::QueueUseCard(CardQueueUse {
                origin,
                card_index,
                card,
                team_type: subscriber.team_type,
                source_skill_id: *release_skill,
                action: Some(action),
            }),
        )));
    }
    ops.push(RuleOp::Command(BattleCommand::Card(
        CardCommand::ResolveCastChannel {
            origin,
            buff_uid: subscriber.buff_uid,
        },
    )));
    Some(ops)
}

fn match_recorded_cards(
    played: &[PlayedCard],
    recorded_skill_ids: &[i32],
    limit: usize,
) -> Vec<CardInfo> {
    let mut played = played.iter();
    recorded_skill_ids
        .iter()
        .take(limit)
        .filter_map(|skill_id| {
            played
                .find(|played| played.card.skill_id == Some(*skill_id))
                .map(|played| played.card.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
            skill_id,
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
}
