use super::*;
use crate::engine::{
    event::kind::EventKind,
    runtime::{determinism::RoundDeterminism, drain::run},
    skill::{
        effect::SkillEffectCatalog,
        subscriber::for_active_buffs,
        target::{TargetContext, TargetPool},
    },
};

fn reflection_fight(buff_id: i32, layers: i32, attacker_hp: i32) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(attacker_hp),
                attr: Some(HeroAttribute {
                    hp: Some(2_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(2_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_800),
                    hp: Some(2_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(35),
                    buff_id: Some(buff_id),
                    from_uid: Some(-1),
                    layer: Some(layers),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn hit() -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
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
    })
}

#[test]
fn thorny_reflects_each_persistent_layer() {
    crate::test_support::init_config();
    let fight = reflection_fight(90021, 3, 2_000);
    let managers = BattleManagers::seeded(&fight);
    let subscriber = for_active_buffs(&managers, EventKind::BeAttacked)
        .into_iter()
        .find(|feature| feature.buff_uid == 35)
        .unwrap();
    let event = hit();
    let ops = rule_ops(&managers, &subscriber, &event).unwrap();
    assert_eq!(ops.len(), 6);
    assert!(ops.chunks_exact(2).all(|layer| matches!(
        layer,
        [
            RuleOp::BuffFeatureMarker {
                target_uid: 10,
                buff_act_id: 303,
                effect_num: 90021,
                ..
            },
            RuleOp::Command(BattleCommand::ReboundDamage(ReflectedDamage(HpDamage {
                target_uid: 10,
                amount: 270,
                ..
            })))
        ]
    )));
}

#[test]
fn single_reflection_ignores_a_restored_layer_field() {
    crate::test_support::init_config();
    let fight = reflection_fight(500401, 3, 2_000);
    let managers = BattleManagers::seeded(&fight);
    let subscriber = for_active_buffs(&managers, EventKind::BeAttacked)
        .into_iter()
        .find(|feature| feature.buff_uid == 35)
        .unwrap();
    let ops = rule_ops(&managers, &subscriber, &hit()).unwrap();
    assert_eq!(ops.len(), 2);
}

#[test]
fn lethal_thorny_keeps_the_second_activation_without_damage() {
    crate::test_support::init_config();
    let fight = reflection_fight(90021, 2, 229);
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [90021]);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Publish(hit())],
    )
    .unwrap();
    let steps = project_for_version(&result.frames, 7).unwrap();
    let reflection = steps
        .iter()
        .find_map(|step| crate::test_support::find_step(step, 90021))
        .unwrap();
    assert_eq!((reflection.from_id, reflection.to_id), (Some(-1), Some(-1)));
    let effects = reflection
        .act_effect
        .iter()
        .filter(|effect| effect.buff_act_id == Some(303))
        .map(|effect| effect.effect_type.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            EffectType::Rebound,
            EffectType::Origindamage,
            EffectType::Rebound
        ]
        .map(|effect| effect as i32)
    );
    assert_eq!(managers.hp.current(10), 0);
}
