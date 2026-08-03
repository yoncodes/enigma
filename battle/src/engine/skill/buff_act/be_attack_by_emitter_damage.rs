use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers, emitter,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, rate, limit]
        if AttrId::from_raw(*raw_attr).is_some() && *rate > 0 && *limit > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [raw_attr, rate, limit] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args)
        || !subscriber.owner_alive
        || hit.source_uid != emitter::UID
        || hit.target_uid != subscriber.owner_uid
        || !managers.can_fire_buff_act(
            subscriber.owner_uid,
            subscriber.buff_uid,
            subscriber.key.definition,
            *limit,
        )
    {
        return Some(Vec::new());
    }

    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(subscriber.source_uid, AttrId::from_raw(*raw_attr)?),
        *rate,
        modifiers::genesis_multiplier(managers, subscriber.source_uid, subscriber.owner_uid),
    ));
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::MarkBuffActFired {
            owner_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            key: subscriber.key.definition,
        },
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: EffectType::Origindamage as i32,
                display_amount: Some(amount),
            }),
        }))),
    ])
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{payload::HitEvent, subscription::SubscriptionKey},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    #[test]
    fn emitter_hit_uses_buff_source_attribute_and_round_cap() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
                        attack: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(5_000),
                    attr: Some(HeroAttribute {
                        hp: Some(5_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let key = DefinitionKey::new(889, "BeAttackByEmitterDamage");
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30480231,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(crate::engine::event::kind::EventKind::BeAttacked, key),
            act_type: "BeAttackByEmitterDamage".to_owned(),
            effect_time: 2091,
            effect_condition: 0,
            args: vec![AttrId::Attack as i32, 1_000, 1],
            raw: "889#102#1000#1".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: emitter::UID,
            target_uid: -1,
            skill_id: 1,
            amount: 100,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });

        let ops = rule_ops(&managers, &subscriber, &event).unwrap();
        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::MarkBuffActFired { .. },
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    source_uid: 10,
                    target_uid: -1,
                    amount: 2_000,
                    ..
                })))
            ]
        ));

        managers.mark_buff_act_fired(-1, 20, key);
        assert!(rule_ops(&managers, &subscriber, &event).unwrap().is_empty());
        managers.begin_round();
        assert!(!rule_ops(&managers, &subscriber, &event).unwrap().is_empty());
    }
}
