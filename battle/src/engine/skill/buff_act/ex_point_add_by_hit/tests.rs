use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    event::{bus::EventBus, kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
    manager::BattleManagers,
    runtime::executor::execute_rule_op,
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
    },
};

use super::*;

#[test]
fn hit_event_emits_manager_commands_with_exact_origin() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11]
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .to_vec(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30620111),
                    from_uid: Some(10),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30620111,
        team_type: 2,
        owner_alive: true,
        amount: 2,
        key: SubscriptionKey::new(
            EventKind::BeAttacked,
            crate::engine::skill::rule::DefinitionKey::new(926, "ExPointAddByHit"),
        ),
        act_type: "ExPointAddByHit".to_owned(),
        effect_time: 209,
        effect_condition: 0,
        args: vec![1],
        raw: "926#1".to_owned(),
    };
    let event = BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 11,
        target_uid: -1,
        skill_id: 100,
        amount: 50,
        resolved_damage: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });

    assert!(super::super::registry::has_destination(
        926,
        "ExPointAddByHit",
        &[1]
    ));
    let scoped = super::super::rule_ops(
        &managers,
        &crate::engine::skill::target::TargetPool::from_fight(&fight),
        &SkillEffectCatalog::default(),
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        &subscriber,
        Some(&event),
    )
    .unwrap();
    assert!(
        scoped
            .iter()
            .all(|entry| { entry.source == super::super::BuffActFrameSource::Applier })
    );
    assert!(matches!(
        scoped[1].op,
        RuleOp::BuffFeatureMarker {
            target_uid: 11,
            buff_act_id: 926,
            ..
        }
    ));
    let ops = rule_ops(&managers, &subscriber, &event).unwrap();
    assert_eq!(ops.len(), 2);
    let mut events = EventBus::default();
    for op in ops {
        execute_rule_op(&mut managers, &mut events, op).unwrap();
    }

    assert_eq!(managers.buff.snapshot(-1, 20).unwrap().layer, Some(1));
    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.ex_point.get(11), 1);
    let mut finished = managers.clone();
    for op in rule_ops(&finished, &subscriber, &event).unwrap() {
        execute_rule_op(&mut finished, &mut events, op).unwrap();
    }
    assert!(finished.buff.snapshot(-1, 20).is_none());
    assert_eq!(finished.ex_point.get(11), 2);

    managers.hp.lose(-1, 100, 0);
    let dead_holder =
        crate::engine::skill::subscriber::for_active_buffs(&managers, EventKind::BeAttacked)
            .into_iter()
            .find(|subscriber| subscriber.buff_uid == 20)
            .unwrap();
    assert!(!dead_holder.owner_alive);
    assert!(
        rule_ops(&managers, &dead_holder, &event)
            .unwrap()
            .is_empty()
    );
    assert_eq!(managers.buff.snapshot(-1, 20).unwrap().layer, Some(1));
    assert_eq!(managers.ex_point.get(11), 1);
}
