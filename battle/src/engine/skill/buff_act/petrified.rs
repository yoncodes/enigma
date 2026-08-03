use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        effect::catalog::{SkillEffectCatalog, SkillEffectTag},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{registry::BuffActKind, subscriber_is_kind};

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !subscriber_is_kind(subscriber, BuffActKind::Petrified) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid
        || catalog.effect_tag(hit.skill_id) != SkillEffectTag::RealityDamage as i32
    {
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
        event::{payload::HitEvent, subscription::SubscriptionKey},
        manager::hp::HurtDamageFromType,
        skill::{
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
            subscriber::BuffActSubscriber,
        },
    };

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 20,
            buff_uid: 30,
            buff_id: 4020,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                crate::engine::event::kind::EventKind::TargetAttacked,
                DefinitionKey::new(402, "Petrified"),
            ),
            act_type: "Petrified".into(),
            effect_time: 0,
            effect_condition: 0,
            args: Vec::new(),
            raw: "402".into(),
        }
    }

    fn hit(skill_id: i32) -> BattleEvent {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(402, "Petrified"),
            },
            source_uid: 20,
            target_uid: 10,
            skill_id,
            amount: 1,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })
    }

    #[test]
    fn only_reality_damage_removes_petrification() {
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_effect_tag(1, SkillEffectTag::RealityDamage as i32);
        catalog.insert_effect_tag(2, SkillEffectTag::MentalDamage as i32);

        assert!(
            rule_ops(&catalog, &subscriber(), &hit(2))
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            rule_ops(&catalog, &subscriber(), &hit(1))
                .unwrap()
                .as_slice(),
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
