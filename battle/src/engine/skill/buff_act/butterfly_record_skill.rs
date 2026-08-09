use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffSetState},
        card::{CardAddPrecast, CardCommand, temp::runtime_precast_card},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !matches!(
        event,
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundStartCard)
    ) {
        return None;
    }
    let act_id = subscriber.key.definition.opcode;
    let record = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)
        .and_then(|state| {
            state
                .act_info
                .iter()
                .find(|info| info.act_id == Some(act_id))
                .and_then(parse_record)
        });
    let Some((count, skill_id)) = record else {
        return Some(Vec::new());
    };
    let enchant_id = *subscriber.args.get(1)?;
    let owner_uid = pool
        .allies(subscriber.owner_uid)
        .iter()
        .find(|entity| {
            entity.skill_group1.contains(&skill_id)
                || entity.skill_group2.contains(&skill_id)
                || crate::engine::mechanic::card::CardMechanic
                    .is_ultimate_skill(managers, skill_id, entity)
        })?
        .uid;
    let origin = super::command_origin(subscriber)?;
    let mut card = runtime_precast_card(managers, owner_uid, skill_id);
    card.enchants.push(sonettobuf::CardEnchant {
        enchant_id: Some(enchant_id),
        duration: Some(1),
        ex_info: Vec::new(),
    });
    let mut ops = (0..count)
        .map(|_| {
            RuleOp::Command(BattleCommand::Card(CardCommand::AddPrecast(
                CardAddPrecast {
                    origin,
                    card: card.clone(),
                },
            )))
        })
        .collect::<Vec<_>>();
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
        BuffSetState {
            origin,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(vec![sonettobuf::BuffActInfo {
                act_id: Some(act_id),
                param: Vec::new(),
                str_param: Some(String::new()),
            }]),
        },
    ))));
    Some(ops)
}

fn parse_record(info: &sonettobuf::BuffActInfo) -> Option<(i32, i32)> {
    let mut values = info.str_param.as_deref()?.split(',');
    let count = values.next()?.parse::<i32>().ok()?;
    let skill_id = values.next()?.parse::<i32>().ok()?;
    (count > 0 && skill_id > 0 && values.next().is_none()).then_some((count, skill_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::rule::DefinitionKey,
    };

    #[test]
    fn record_state_requires_count_and_skill() {
        let info = sonettobuf::BuffActInfo {
            str_param: Some("5,31390111".into()),
            ..Default::default()
        };
        assert_eq!(parse_record(&info), Some((5, 31390111)));
    }

    #[test]
    fn empty_record_is_a_valid_round_start_noop() {
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStartCard,
                DefinitionKey::new(1104, "ButterflyRecordSkill"),
            ),
            act_type: "ButterflyRecordSkill".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            args: vec![1, 218, 1],
            raw: "1104#1#218#1".to_owned(),
        };

        assert!(
            rule_ops(
                &BattleManagers::default(),
                &TargetPool::default(),
                &subscriber,
                &BattleEvent::Kind(EventKind::RoundStartCard),
            )
            .is_some_and(|ops| ops.is_empty())
        );
    }
}
