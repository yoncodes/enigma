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
        resolved_damage: 10,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
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

    let mut killed = managers;
    killed.hp.lose(20, 300, 0);
    assert!(matches!(
        rule_ops(&killed, &subscriber, &event),
        Some(ops) if ops.is_empty()
    ));
}
