use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        buff::{ActiveBuffFeature, BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::HpManager,
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [permille] if *permille > 0)
}

pub fn cap(feature: &ActiveBuffFeature, hp: &HpManager) -> Option<i32> {
    if !is_kind(feature, BuffActKind::DamageNotMoreThan) {
        return None;
    }
    let [_, permille] = feature.values.as_slice() else {
        return None;
    };
    Some(
        (i64::from(hp.max(feature.owner_uid)) * i64::from(*permille) / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32,
    )
}

pub fn consume_after_hit(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::DamageNotMoreThan) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if hit.target_uid != subscriber.owner_uid
        || config::try_get()
            .and_then(|db| db.skill_buff.get(subscriber.buff_id))
            .is_none_or(|buff| buff.effect_count <= 0)
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Consume(BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
        manager::hp::HurtDamageFromType,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    fn subscriber(buff_id: i32) -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::TargetAttacked,
                DefinitionKey::new(510, "DamageNotMoreThan"),
            ),
            act_type: "DamageNotMoreThan".to_owned(),
            effect_time: 207,
            effect_condition: 0,
            args: vec![100],
            raw: "510#100".to_owned(),
        }
    }

    fn hit(target_uid: i64) -> BattleEvent {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: -1,
            target_uid,
            skill_id: 1,
            amount: 100,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })
    }

    #[test]
    fn stacked_cap_consumes_one_layer_after_its_owner_is_hit() {
        crate::test_support::init_config();

        assert!(
            consume_after_hit(&subscriber(6240530), &hit(11))
                .unwrap()
                .is_empty()
        );
        assert!(
            consume_after_hit(&subscriber(610091), &hit(10))
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            consume_after_hit(&subscriber(6240530), &hit(10))
                .unwrap()
                .as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    target_uid: 10,
                    selector: BuffSelector::Uid(20),
                    amount: 1,
                    depleted: DepletedBuff::Remove,
                    ..
                }
            )))]
        ));

        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(6240530),
                        from_uid: Some(10),
                        count: Some(1),
                        layer: Some(3),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let RuleOp::Command(BattleCommand::Buff(command)) =
            consume_after_hit(&subscriber(6240530), &hit(10))
                .unwrap()
                .pop()
                .unwrap()
        else {
            panic!("expected buff consumption");
        };
        let changes = managers.buff.execute(&managers.hp, command).unwrap();

        assert_eq!(changes.change.refreshed[0].before.layer, Some(3));
        assert_eq!(changes.change.refreshed[0].after.layer, Some(2));
        assert_eq!(changes.change.refreshed[0].after.count, Some(1));
    }
}
