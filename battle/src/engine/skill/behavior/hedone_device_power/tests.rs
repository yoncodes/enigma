use sonettobuf::{
    ActEffect, BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightStep, FightTeam,
    effect_type_enum::EffectType,
};

use crate::engine::{
    manager::BattleManagers,
    packet::timeline,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, classify::BehaviorSpec, registry::BehaviorHandler},
        effect::{ParsedBehavior, SkillEffectCatalog},
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool},
    },
};

use super::*;

fn effects<'a>(steps: &'a [FightStep], output: &mut Vec<&'a ActEffect>) {
    for step in steps {
        for effect in &step.act_effect {
            output.push(effect);
            if let Some(child) = &effect.fight_step {
                effects(std::slice::from_ref(child), output);
            }
        }
    }
}

fn carrier(uid: i64, buff_id: i32, state: Vec<i32>) -> BuffInfo {
    BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(-4),
        act_info: vec![BuffActInfo {
            act_id: Some(1159),
            param: state,
            str_param: Some(String::new()),
        }],
        ..Default::default()
    }
}

fn fight(buffs: Vec<BuffInfo>) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(3149),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-4),
                    model_id: Some(3156),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    buffs,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn behavior() -> ParsedBehavior {
    ParsedBehavior::from_spec(
        BehaviorSpec::new(60334, "HedoneAddDevicePower"),
        vec![1],
        vec!["1".into()],
    )
}

fn ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    behavior: &ParsedBehavior,
    determinism: &mut RoundDeterminism,
) -> Vec<RuleOp> {
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 1,
            target_uid: -4,
            active_skill_id: 31560181,
            transfer_count: 1,
            event: None,
            managers,
            pool,
            determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        behavior,
    )
    .unwrap()
}

#[test]
fn carrier_state_owns_the_power_change() {
    crate::test_support::init_config();
    let fight = fight(vec![carrier(1108, 31560187, vec![4, 12, 1])]);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = behavior();
    let mut determinism = RoundDeterminism::default();
    let ops = ops(&managers, &pool, &behavior, &mut determinism);
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::HedoneDevicePower(
            crate::engine::mechanic::hedone_device_power::HedoneDevicePowerCommand {
                owner_uid: -4,
                power_id: 1,
                ..
            }
        ))]
    ));
    let result = drain::run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();

    assert_eq!(managers.conduit.power(1, 1), 8);
    let projected = timeline::project_for_version(&result.frames, 7).unwrap();
    let mut projected_effects = Vec::new();
    effects(&projected, &mut projected_effects);
    let [change] = projected_effects.as_slice() else {
        panic!("Hedone power change must project one effect")
    };
    assert_eq!(
        change.effect_type,
        Some(EffectType::Devicepowerchange as i32)
    );
    assert_eq!(change.effect_num, Some(0));
    assert_eq!(change.reserve_str.as_deref(), Some("1#8"));
    assert_eq!(change.team_type, Some(1));
}

#[test]
fn invalid_carriers_fail_in_the_runtime_without_granting_power() {
    crate::test_support::init_config();
    let cases = [
        Vec::new(),
        vec![carrier(1108, 31560187, vec![4, 12])],
        vec![
            carrier(1108, 31560187, vec![4, 12, 1]),
            carrier(1109, 31560186, vec![4, 12, 1]),
        ],
    ];
    for buffs in cases {
        let fight = fight(buffs);
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut determinism = RoundDeterminism::default();
        let ops = ops(&managers, &pool, &behavior(), &mut determinism);
        assert!(
            drain::run_command_group(
                &mut managers,
                &pool,
                &SkillEffectCatalog::default(),
                &mut determinism,
                TargetContext::default(),
                ops,
            )
            .is_err()
        );
        assert_eq!(managers.conduit.power(1, 1), 0);
    }
}

#[test]
fn grammar_requires_one_positive_power_type() {
    assert!(Handler::supports(&ParsedBehavior::new(
        60334,
        "HedoneAddDevicePower",
        vec![1],
    )));
    for args in [vec![], vec![0], vec![-1], vec![1, 1]] {
        assert!(!Handler::supports(&ParsedBehavior::new(
            60334,
            "HedoneAddDevicePower",
            args,
        )));
    }
}
