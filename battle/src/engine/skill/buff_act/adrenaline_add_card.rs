use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand},
        card::{CardAddGenerated, CardAddTemporary, CardCommand, TemporaryCardKind},
        ex_point::{ExPointCommand, ExPointSet},
    },
    skill::{
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AdrenalineAddCard)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
    {
        return None;
    }
    let (thresholds, skill_ids) = parse(&subscriber.args)?;
    let progress = usize::try_from(
        managers
            .buff
            .act_value(subscriber.buff_uid, subscriber.key.definition.opcode),
    )
    .ok()?;
    let (&threshold, &skill_id) = thresholds.get(progress).zip(skill_ids.get(progress))?;
    if managers.ex_point.get(subscriber.owner_uid) < threshold {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let terminal = progress + 1 == thresholds.len();
    let mut ops = Vec::with_capacity(3);
    if terminal {
        ops.push(RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Set(ExPointSet {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                value: 0,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            }),
        )));
    }
    if progress > 0 || !terminal {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta: if terminal { -(progress as i32) } else { 1 },
            }),
        )));
    }
    ops.push(RuleOp::Command(BattleCommand::Card(if terminal {
        CardCommand::AddGenerated(CardAddGenerated {
            origin,
            target_uid: subscriber.owner_uid,
            skill_id,
            hero_id: Some(managers.entity.model_id(subscriber.owner_uid)?),
            team_type: subscriber.team_type,
        })
    } else {
        CardCommand::AddTemporary(CardAddTemporary {
            origin,
            target_uid: subscriber.owner_uid,
            skill_id,
            hero_id: Some(managers.entity.model_id(subscriber.owner_uid)?),
            reserve_id: 0,
            team_type: subscriber.team_type,
            kind: TemporaryCardKind::ConfiguredSkill3,
        })
    })));
    Some(ops)
}

pub fn supports(args: &[i32]) -> bool {
    parse(args).is_some()
}

fn parse(args: &[i32]) -> Option<(&[i32], &[i32])> {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return None;
    }
    let (thresholds, skill_ids) = args.split_at(args.len() / 2);
    (thresholds.iter().all(|threshold| *threshold > 0)
        && thresholds.windows(2).all(|pair| pair[0] < pair[1])
        && skill_ids.iter().all(|skill_id| *skill_id > 0))
    .then_some((thresholds, skill_ids))
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey};

    #[test]
    fn below_threshold_does_not_add_the_configured_card() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3124),
                    current_hp: Some(100),
                    ex_point: Some(1),
                    ex_point_type: Some(3),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31242140),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31242140,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStartCard,
                DefinitionKey::new(10001, "AdrenalineAddCard"),
            ),
            act_type: "AdrenalineAddCard".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            args: vec![2, 6, 10, 312451011, 312451023, 312451031],
            raw: "10001#2,6,10#312451011,312451023,312451031".to_owned(),
        };

        let ops = rule_ops(
            &BattleManagers::seeded(&fight),
            &subscriber,
            &BattleEvent::Kind(EventKind::RoundStartCard),
        )
        .unwrap();

        assert!(ops.is_empty());
        assert!(supports(&[10, 31242103]));
        assert!(supports(&[2, 6, 10, 312451011, 312451023, 312451031]));
        assert!(!supports(&[10]));
        assert!(!supports(&[6, 2, 312451023, 312451011]));
    }

    #[test]
    fn grouped_thresholds_advance_temporary_cards_then_reset_with_the_terminal_card() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3124),
                    current_hp: Some(100),
                    ex_point: Some(2),
                    ex_point_type: Some(3),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31242140),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31242140,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStartCard,
                DefinitionKey::new(10001, "AdrenalineAddCard"),
            ),
            act_type: "AdrenalineAddCard".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            args: vec![2, 6, 10, 312451011, 312451023, 312451031],
            raw: "10001#2,6,10#312451011,312451023,312451031".to_owned(),
        };
        let event = BattleEvent::Kind(EventKind::RoundStartCard);
        let mut managers = BattleManagers::seeded(&fight);
        let origin = super::super::command_origin(&subscriber).unwrap();

        let first = rule_ops(&managers, &subscriber, &event).unwrap();
        assert!(matches!(
            first.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                    BuffAccumulateActValue { delta: 1, .. }
                ))),
                RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(
                    CardAddTemporary {
                        skill_id: 312451011,
                        hero_id: Some(3124),
                        kind: TemporaryCardKind::ConfiguredSkill3,
                        ..
                    }
                )))
            ]
        ));
        let RuleOp::Command(BattleCommand::Buff(command)) = &first[0] else {
            unreachable!()
        };
        managers.execute_buff(command.clone()).unwrap();
        assert!(rule_ops(&managers, &subscriber, &event).unwrap().is_empty());
        managers
            .execute_ex_point(ExPointCommand::Set(ExPointSet {
                origin,
                source_uid: 10,
                target_uid: 10,
                value: 6,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            }))
            .unwrap();

        let second = rule_ops(&managers, &subscriber, &event).unwrap();
        assert!(matches!(
            second.last(),
            Some(RuleOp::Command(BattleCommand::Card(
                CardCommand::AddTemporary(CardAddTemporary {
                    skill_id: 312451023,
                    ..
                })
            )))
        ));
        let RuleOp::Command(BattleCommand::Buff(command)) = &second[0] else {
            unreachable!()
        };
        managers.execute_buff(command.clone()).unwrap();
        assert!(rule_ops(&managers, &subscriber, &event).unwrap().is_empty());
        managers
            .execute_ex_point(ExPointCommand::Set(ExPointSet {
                origin,
                source_uid: 10,
                target_uid: 10,
                value: 10,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            }))
            .unwrap();

        let terminal = rule_ops(&managers, &subscriber, &event).unwrap();

        assert!(matches!(
            terminal.as_slice(),
            [
                RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Set(ExPointSet {
                    target_uid: 10,
                    value: 0,
                    ..
                }))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                    BuffAccumulateActValue { delta: -2, .. }
                ))),
                RuleOp::Command(BattleCommand::Card(CardCommand::AddGenerated(
                    CardAddGenerated {
                        target_uid: 10,
                        skill_id: 312451031,
                        hero_id: Some(3124),
                        team_type: 1,
                        ..
                    }
                )))
            ]
        ));
        let RuleOp::Command(BattleCommand::ExPoint(command)) = &terminal[0] else {
            unreachable!()
        };
        managers.execute_ex_point(*command).unwrap();
        let RuleOp::Command(BattleCommand::Buff(command)) = &terminal[1] else {
            unreachable!()
        };
        managers.execute_buff(command.clone()).unwrap();
        let RuleOp::Command(BattleCommand::Card(command)) = &terminal[2] else {
            unreachable!()
        };
        let changes = managers.execute_card(command.clone()).unwrap();
        assert_eq!(managers.ex_point.get(10), 0);
        assert_eq!(managers.buff.act_value(20, 10001), 0);
        assert_eq!(
            changes.kind,
            crate::engine::manager::card::CardChangeKind::OwnedGeneratedAdded
        );
        let card = changes.added.unwrap();
        assert_eq!(card.hero_id, Some(3124));
        assert_eq!(card.temp_card, Some(false));
    }
}
