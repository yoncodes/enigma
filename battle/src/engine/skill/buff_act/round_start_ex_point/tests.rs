use super::*;
use crate::engine::{
    event::subscription::SubscriptionKey,
    manager::BattleManagers,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        buff_act::{self, registry},
        effect::SkillEffectCatalog,
        rule::DefinitionKey,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn round_start_gain_uses_the_holder() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3108),
                    current_hp: Some(100),
                    ex_point: Some(4),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(22300962),
                        from_uid: Some(20),
                        duration: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::default();
    let mut determinism = RoundDeterminism::default();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 20,
        buff_uid: 30,
        buff_id: 22300962,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(823, "ExPointAdd"),
        ),
        act_type: "ExPointAdd".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![1],
        raw: "823#1".to_owned(),
    };
    assert!(
        rule_ops(&subscriber, &BattleEvent::RoundStart)
            .unwrap()
            .is_empty()
    );
    let ops = buff_act::rule_ops(
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &subscriber,
        Some(&BattleEvent::Kind(EventKind::RoundStartCard)),
    )
    .unwrap();
    assert!(matches!(ops[0].op, RuleOp::BuffFeatureMarker {
        target_uid: 10, effect_num: 22300962, buff_act_id: 823, effect_type,
    } if effect_type == EffectType::Expointadd as i32));
    let result = drain::run_round_start_card_event(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        &[10, 20],
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(managers.ex_point.get(20), 0);
    let steps = crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
    let gain = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| {
            step.act_effect
                .iter()
                .any(|effect| effect.buff_act_id == Some(823))
        })
        .expect("discovered round-start Moxie subscriber");
    assert_eq!(gain.from_id, Some(10));
    assert_eq!(gain.to_id, Some(10));
    assert_eq!(gain.act_id, Some(22300962));
    let marker = gain
        .act_effect
        .iter()
        .position(|effect| effect.buff_act_id == Some(823))
        .unwrap();
    assert_eq!(
        gain.act_effect
            .iter()
            .skip(marker)
            .take(2)
            .map(|effect| (effect.effect_type, effect.target_id, effect.effect_num))
            .collect::<Vec<_>>(),
        vec![
            (
                Some(EffectType::Expointadd as i32),
                Some(10),
                Some(22300962)
            ),
            (Some(EffectType::Expointchange as i32), Some(10), Some(1)),
        ]
    );
}

#[test]
fn exact_rule_keeps_round_start_and_runtime_marker() {
    let definition = registry::find(823, "ExPointAdd").unwrap();
    assert_eq!(
        definition.runtime.event_override,
        Some(EventKind::RoundStartCard)
    );
    let supports = definition.supports.unwrap();
    assert!(supports(&[1]));
    for args in [vec![], vec![0], vec![-1], vec![2], vec![1, 1]] {
        assert!(!supports(&args));
    }
    assert!(registry::find(823, "ExPointDel").is_none());
    let wire = definition.wire.unwrap();
    assert!(wire.markers(buff_act::wire::WirePhase::Add).is_empty());
    assert!(wire.markers(buff_act::wire::WirePhase::Refresh).is_empty());
    assert_eq!(
        wire.markers(buff_act::wire::WirePhase::Static),
        &[EffectType::Expointadd as i32]
    );
}
