use crate::engine::{
    entity::skill::skill_rank,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
        card::{CardAddTemporary, CardCommand, TemporaryCardKind},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

use super::{is_kind, registry::BuffActKind};

pub fn skill_id(feature: &ActiveBuffFeature) -> Option<i32> {
    if !is_kind(feature, BuffActKind::AddSpTempCard) {
        return None;
    }
    let [_, skill_id, ..] = feature.values.as_slice() else {
        return None;
    };
    (*skill_id > 0).then_some(*skill_id)
}

pub fn rule_op(feature: &ActiveBuffFeature, target_uid: i64, reserve_id: i64) -> Option<RuleOp> {
    Some(RuleOp::Command(BattleCommand::Card(
        CardCommand::AddTemporary(CardAddTemporary {
            origin: super::feature_command_origin(feature)?,
            target_uid,
            skill_id: skill_id(feature)?,
            reserve_id,
            team_type: feature.team_type,
            kind: TemporaryCardKind::ConfiguredSkill,
        }),
    )))
}

pub fn subscriber_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    reserve_id: i64,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddSpTempCard)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
    {
        return None;
    }
    let [skill_id, ..] = subscriber.args.as_slice() else {
        return None;
    };
    if *skill_id <= 0 || reserve_id <= 0 {
        return None;
    }
    temporary_card_rule_ops(
        subscriber,
        *skill_id,
        reserve_id,
        TemporaryCardKind::ConfiguredSkill,
    )
}

pub fn supports_hero_skill(args: &[i32]) -> bool {
    matches!(args, [group, rank, 1] if matches!(group, 1 | 2) && (1..=3).contains(rank))
}

pub fn hero_skill_subscriber_rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CreateHeroTempCards)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
        || !supports_hero_skill(&subscriber.args)
    {
        return None;
    }
    let [group, rank, 1] = subscriber.args.as_slice() else {
        return None;
    };
    let owner = pool.entity(subscriber.owner_uid)?;
    let skills = if *group == 1 {
        &owner.skill_group1
    } else {
        &owner.skill_group2
    };
    let skill_id = skills
        .iter()
        .copied()
        .find(|skill_id| skill_rank(*skill_id) == *rank)?;
    temporary_card_rule_ops(
        subscriber,
        skill_id,
        i64::from(owner.model_id),
        TemporaryCardKind::HeroSkill,
    )
}

fn temporary_card_rule_ops(
    subscriber: &BuffActSubscriber,
    skill_id: i32,
    reserve_id: i64,
    kind: TemporaryCardKind,
) -> Option<Vec<super::BuffActRuleOp>> {
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        super::BuffActRuleOp::subscriber_from_owner(RuleOp::Command(BattleCommand::Card(
            CardCommand::AddTemporary(CardAddTemporary {
                origin,
                target_uid: subscriber.owner_uid,
                skill_id,
                reserve_id,
                team_type: subscriber.team_type,
                kind,
            }),
        ))),
        super::BuffActRuleOp::separate_independent_command(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::RemoveAfterTrigger(BuffRemove {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            }),
        ))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey},
        test_support::init_config,
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    #[test]
    fn exact_feature_emits_a_configured_temporary_card_command() {
        let feature = ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "AddSpTempCard".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "815#999".to_owned(),
            values: vec![815, 999],
        };

        assert!(matches!(
            rule_op(&feature, 10, 40),
            Some(RuleOp::Command(BattleCommand::Card(
                CardCommand::AddTemporary(CardAddTemporary {
                    target_uid: 10,
                    skill_id: 999,
                    reserve_id: 40,
                    team_type: 1,
                    ..
                })
            )))
        ));
    }

    #[test]
    fn round_start_card_consumes_the_buff_after_adding_its_configured_card() {
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
                DefinitionKey::new(815, "AddSpTempCard"),
            ),
            act_type: "AddSpTempCard".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            args: vec![999],
            raw: "815#999".to_owned(),
        };

        let ops = subscriber_rule_ops(
            &subscriber,
            &BattleEvent::Kind(EventKind::RoundStartCard),
            3114,
        )
        .unwrap();

        assert!(matches!(
            &ops[0].op,
            RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
                if add.skill_id == 999 && add.target_uid == 10 && add.reserve_id == 3114
        ));
        assert!(matches!(
            &ops[1].op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(remove)))
                if remove.target_uid == 10
                    && remove.selector == BuffRemoveSelector::Uid(20)
        ));
        assert_eq!(
            ops[0].scope,
            super::super::BuffActFrameScope::SubscriberFrame
        );
        assert_eq!(ops[0].source, super::super::BuffActFrameSource::Owner);
        assert!(!ops[1].group_with_siblings);
        assert_eq!(ops[1].frame_owner, super::super::BuffActFrameOwner::Command);
    }

    #[test]
    fn hero_card_group_and_rank_select_the_captured_precast() {
        init_config();
        let pool = TargetPool::from_fight(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3070),
                    skill_group1: vec![307001172, 307001182, 307001192],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 307002612,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStartCard,
                DefinitionKey::new(739, "CreateHeroTempCards"),
            ),
            act_type: "CreateHeroTempCards".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            args: vec![1, 2, 1],
            raw: "739#1#2#1".to_owned(),
        };

        let ops = hero_skill_subscriber_rule_ops(
            &pool,
            &subscriber,
            &BattleEvent::Kind(EventKind::RoundStartCard),
        )
        .unwrap();

        assert!(matches!(
            &ops[0].op,
            RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
                if add.skill_id == 307001182
                    && add.target_uid == 10
                    && add.reserve_id == 3070
        ));
        assert!(!supports_hero_skill(&[0, 2, 1]));
        assert!(!supports_hero_skill(&[1, 2, 2]));
    }
}
