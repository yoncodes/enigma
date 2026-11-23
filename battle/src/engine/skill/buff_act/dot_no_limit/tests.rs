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

fn fight() -> Fight {
    Fight {
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
    }
}

fn managers() -> BattleManagers {
    BattleManagers::seeded(&fight())
}

fn hit(damage_from: HurtDamageFromType) -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
        target_uid: 20,
        skill_id: 1,
        amount: 100,
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn mode_selects_applier_or_holder_attribute() {
    let managers = managers();
    let event = hit(HurtDamageFromType::Skill);
    let amount = |mode| {
        rule_ops(&managers, &subscriber(mode), &event)
            .unwrap()
            .into_iter()
            .find_map(|op| match op {
                BuffActRuleOp {
                    op: RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))),
                    source: BuffActFrameSource::Applier,
                    ..
                } => Some(damage.amount),
                _ => None,
            })
            .unwrap_or_default()
    };

    assert_eq!((amount(0), amount(1)), (200, 400));
}

#[test]
fn additional_damage_is_not_another_attack() {
    let managers = managers();

    assert_eq!(
        rule_ops(
            &managers,
            &subscriber(0),
            &hit(HurtDamageFromType::Additional)
        )
        .map(|ops| ops.len()),
        Some(0)
    );
}

#[test]
fn defeated_holder_still_reports_the_trigger_without_damage() {
    let managers = managers();
    let mut subscriber = subscriber(0);
    subscriber.owner_alive = false;

    assert!(matches!(
        rule_ops(&managers, &subscriber, &hit(HurtDamageFromType::Skill))
            .unwrap()
            .as_slice(),
        [BuffActRuleOp {
            op: RuleOp::BuffFeatureMarker {
                target_uid: 20,
                effect_type,
                effect_num: 530000412,
                buff_act_id: 721,
            },
            source: BuffActFrameSource::Applier,
            ..
        }] if *effect_type == EffectType::Dot as i32
    ));
}

#[test]
fn defeated_holder_still_rejects_malformed_arguments() {
    let managers = managers();
    for args in [vec![2, AttrId::Attack as i32, 200], vec![0, i32::MAX, 200]] {
        let mut subscriber = subscriber(0);
        subscriber.owner_alive = false;
        subscriber.args = args;
        assert!(rule_ops(&managers, &subscriber, &hit(HurtDamageFromType::Skill)).is_none());
    }
}

#[test]
fn zero_damage_still_reports_the_trigger() {
    let managers = managers();
    let mut subscriber = subscriber(0);
    subscriber.args[2] = 0;

    assert!(matches!(
        rule_ops(&managers, &subscriber, &hit(HurtDamageFromType::Skill))
            .unwrap()
            .as_slice(),
        [BuffActRuleOp {
            op: RuleOp::BuffFeatureMarker {
                buff_act_id: 721,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn registry_path_does_not_duplicate_owned_markers() {
    let fight = fight();
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let event = hit(HurtDamageFromType::Skill);
    let ops = |owner_alive| {
        let mut subscriber = subscriber(0);
        subscriber.owner_alive = owner_alive;
        super::super::rule_ops(
            &managers,
            &pool,
            &crate::engine::skill::effect::SkillEffectCatalog::default(),
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &subscriber,
            Some(&event),
        )
        .unwrap()
    };

    for (owner_alive, damage_count) in [(true, 1), (false, 0)] {
        let ops = ops(owner_alive);
        assert_eq!(
            ops.iter()
                .filter(|op| matches!(op.op, RuleOp::BuffFeatureMarker { .. }))
                .count(),
            1
        );
        assert_eq!(
            ops.iter()
                .filter(|op| matches!(op.op, RuleOp::Command(BattleCommand::Hp(_))))
                .count(),
            damage_count
        );
    }
}
