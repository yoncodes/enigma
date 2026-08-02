use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [mode, raw_attr, permille] = subscriber.args.as_slice() else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid || !subscriber.owner_alive {
        return Some(Vec::new());
    }
    let attr_uid = match mode {
        0 => subscriber.source_uid,
        1 => subscriber.owner_uid,
        _ => return None,
    };
    let attr_uid = if attr_uid == 0 {
        subscriber.owner_uid
    } else {
        attr_uid
    };
    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(attr_uid, AttrId::from_raw(*raw_attr)?),
        (*permille).max(0),
        modifiers::genesis_multiplier(managers, subscriber.source_uid, subscriber.owner_uid),
    ));
    if amount <= 0 {
        return Some(Vec::new());
    }
    Some(vec![super::BuffActRuleOp::subscriber_from_applier(
        RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            effect_kind: DamageEffectKind::Genesis,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
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
            },
        }))),
    )])
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::payload::HitEvent,
        skill::{
            buff_act::{BuffActFrameSource, BuffActRuleOp},
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
            subscriber::BuffActSubscriber,
        },
    };

    fn subscriber(mode: i32) -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 20,
            source_uid: 10,
            buff_uid: 82,
            buff_id: 530000412,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::BeAttacked,
                DefinitionKey::new(721, "DotNoLimit"),
            ),
            act_type: "DotNoLimit".to_owned(),
            effect_time: 2091,
            effect_condition: 3,
            args: vec![mode, AttrId::Attack as i32, 200],
            raw: format!("721#{mode}#102#200"),
        }
    }

    #[test]
    fn mode_selects_applier_or_holder_attribute() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        attack: Some(1_000),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        attack: Some(2_000),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: 20,
            skill_id: 1,
            amount: 100,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });
        let amount = |mode| match rule_ops(&managers, &subscriber(mode), &event)
            .unwrap()
            .as_slice()
        {
            [
                BuffActRuleOp {
                    op: RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))),
                    source: BuffActFrameSource::Applier,
                    ..
                },
            ] => damage.amount,
            _ => 0,
        };

        assert_eq!((amount(0), amount(1)), (200, 400));
    }
}
