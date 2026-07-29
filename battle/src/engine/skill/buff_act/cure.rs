use crate::engine::{
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffDurationAdvance},
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let kind = super::subscriber_kind(subscriber)?;
    if !supports(kind, &subscriber.args) {
        return None;
    }
    let attacked = matches!(event, BattleEvent::Hit(_));
    match kind {
        BuffActKind::Cure
            if !matches!(
                event,
                BattleEvent::RoundStart | BattleEvent::Kind(EventKind::RoundStart)
            ) =>
        {
            return Some(Vec::new());
        }
        BuffActKind::AdvancedCure
            if !matches!(
                event,
                BattleEvent::RoundStart
                    | BattleEvent::Kind(EventKind::RoundStart)
                    | BattleEvent::Hit(_)
            ) =>
        {
            return Some(Vec::new());
        }
        _ => {}
    }
    if attacked
        && (event.target_uid() != Some(subscriber.owner_uid)
            || managers
                .hp
                .current(subscriber.owner_uid)
                .saturating_mul(1000)
                >= managers
                    .hp
                    .max(subscriber.owner_uid)
                    .saturating_mul(subscriber.args[3]))
    {
        return Some(Vec::new());
    }
    let Some(command) = heal_command(managers, subscriber, subscriber.key.definition.opcode) else {
        return Some(Vec::new());
    };
    let mut ops = vec![RuleOp::Command(BattleCommand::Hp(command))];
    if attacked {
        let take_stage = managers
            .buff
            .duration_stage(subscriber.owner_uid, subscriber.buff_uid)?;
        let advance = BuffDurationAdvance::new(
            take_stage,
            vec![subscriber.owner_uid],
            Some(vec![subscriber.buff_uid]),
        )?;
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AdvanceDuration(advance),
        )));
    }
    Some(ops)
}

pub(crate) fn heal_command(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    config_effect: i32,
) -> Option<HpCommand> {
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let kind = super::subscriber_kind(subscriber)?;
    let base = match kind {
        BuffActKind::Cure => subscriber
            .args
            .chunks_exact(3)
            .try_fold(0_i32, |sum, chunk| {
                let attr = AttrId::from_raw(chunk[1])?;
                Some(
                    sum.saturating_add(
                        managers
                            .origin_attribute(subscriber.owner_uid, attr)
                            .max(0)
                            .saturating_mul(chunk[2])
                            / 1000,
                    ),
                )
            })?,
        BuffActKind::AdvancedCure => {
            let [_mode, raw_attr, permille, _threshold] = subscriber.args.as_slice() else {
                return None;
            };
            managers
                .origin_attribute(source_uid, AttrId::from_raw(*raw_attr)?)
                .max(0)
                .saturating_mul(*permille)
                / 1000
        }
        _ => return None,
    };
    let amount = crate::engine::damage::handler::modified_heal(
        base,
        source_uid,
        subscriber.owner_uid,
        managers,
    );
    (amount > 0).then(|| {
        HpCommand::Heal(HpHeal {
            origin: super::command_origin(subscriber).expect("registered Cure buff act"),
            source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect,
            kind: HpHealKind::Normal,
        })
    })
}

pub fn supports(kind: BuffActKind, args: &[i32]) -> bool {
    match kind {
        BuffActKind::Cure => {
            !args.is_empty()
                && args.len().is_multiple_of(3)
                && args
                    .chunks_exact(3)
                    .all(|chunk| AttrId::from_raw(chunk[1]).is_some() && chunk[2] > 0)
        }
        BuffActKind::AdvancedCure => matches!(
            args,
            [_, raw_attr, permille, threshold]
                if AttrId::from_raw(*raw_attr).is_some() && *permille > 0 && *threshold > 0
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{payload::HitEvent, subscription::SubscriptionKey},
        manager::hp::HurtDamageFromType,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    #[test]
    fn periodic_cure_uses_the_owners_effective_attack() {
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
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
            buff_id: 600104,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(EventKind::RoundStart, DefinitionKey::new(201, "Cure")),
            act_type: "Cure".to_owned(),
            effect_time: 102,
            effect_condition: 0,
            args: vec![0, AttrId::Attack as i32, 500],
            raw: "201#0#102#500".to_owned(),
        };

        assert!(matches!(
            rule_ops(&managers, &subscriber, &BattleEvent::RoundStart),
            Some(ops) if matches!(
                ops.as_slice(),
                [RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                    amount: 500,
                    config_effect: 201,
                    ..
                })))]
            )
        ));
    }

    #[test]
    fn advanced_cure_uses_source_attack_and_advances_only_its_buff_when_hit_at_low_hp() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(4_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        current_hp: Some(300),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: vec![sonettobuf::BuffInfo {
                            uid: Some(30),
                            buff_id: Some(30091111),
                            from_uid: Some(10),
                            duration: Some(2),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });
        let subscriber = BuffActSubscriber {
            owner_uid: 20,
            source_uid: 10,
            buff_uid: 30,
            buff_id: 30091111,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::BeAttacked,
                DefinitionKey::new(849, "AdvancedCure"),
            ),
            act_type: "AdvancedCure".to_owned(),
            effect_time: 102,
            effect_condition: 0,
            args: vec![2, AttrId::Attack as i32, 500, 400],
            raw: "849#2#102#500#400".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: -1,
            target_uid: 20,
            skill_id: 1,
            amount: 10,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
        });

        assert!(matches!(
            rule_ops(&managers, &subscriber, &event),
            Some(ops) if matches!(
                ops.as_slice(),
                [
                    RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                        amount: 2_000,
                        config_effect: 849,
                        source_uid: 10,
                        target_uid: 20,
                        ..
                    }))),
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::AdvanceDuration(
                        BuffDurationAdvance {
                            take_stage: 103,
                            buff_uids,
                            ..
                        }
                    )))
                ] if buff_uids.as_deref() == Some([30].as_slice())
            )
        ));

        let mut inactive = subscriber.clone();
        inactive.owner_uid = 10;
        inactive.buff_uid = 31;
        let BattleEvent::Hit(hit) = event else {
            unreachable!()
        };
        let inactive_event = BattleEvent::Hit(HitEvent {
            target_uid: 10,
            ..hit
        });
        assert!(matches!(
            rule_ops(&managers, &inactive, &inactive_event),
            Some(ops) if ops.is_empty()
        ));
    }
}
