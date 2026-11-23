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
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
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
