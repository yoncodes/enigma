use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{registry::BuffActKind, subscriber_is_kind};

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !subscriber_is_kind(subscriber, BuffActKind::Sleep) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Remove(BuffRemove {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }),
    ))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::payload::HitEvent,
        manager::hp::HurtDamageFromType,
        skill::{
            rule::{CommandOrigin, DefinitionKey},
            subscriber::BuffActSubscriber,
        },
    };

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 20,
            buff_uid: 30,
            buff_id: 4031,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::TargetAttacked,
                DefinitionKey::new(403, "Sleep"),
            ),
            act_type: "Sleep".into(),
            effect_time: 0,
            effect_condition: 0,
            args: Vec::new(),
            raw: "403".into(),
        }
    }

    fn hit(target_uid: i64) -> BattleEvent {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::BuffAct,
                key: DefinitionKey::new(403, "Sleep"),
            },
            source_uid: 20,
            target_uid,
            skill_id: 1,
            amount: 1,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })
    }

    #[test]
    fn attacked_holder_removes_the_exact_sleep_buff() {
        assert!(rule_ops(&subscriber(), &hit(11)).unwrap().is_empty());
        assert!(matches!(
            rule_ops(&subscriber(), &hit(10)).unwrap().as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                BuffRemove {
                    target_uid: 10,
                    selector: BuffRemoveSelector::Uid(30),
                    ..
                }
            )))]
        ));
    }
}
