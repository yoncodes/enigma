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
    let BattleEvent::ActionQueueCommitted {
        cards: _,
        action_count,
        ..
    } = event
    else {
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
        let card_index = *action_count as i32 + offset as i32 + 1;
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
                reserve_id: 0,
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
mod tests;
