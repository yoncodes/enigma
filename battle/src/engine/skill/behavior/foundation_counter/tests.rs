use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffSetState},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{self, classify::BehaviorSpec},
        target::{TargetContext, TargetPool},
    },
};

fn fight(current: i32) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(5_000),
                    attr: Some(HeroAttribute {
                        hp: Some(5_000),
                        attack: Some(2_121),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31_400_003),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1147),
                            param: vec![current, 10_605],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(5_000),
                    attr: Some(HeroAttribute {
                        hp: Some(5_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn evaluate(fight: Fight) -> (Option<Vec<RuleOp>>, SkillModifiers) {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60307, "FoundationCounterClear"),
        vec![2_500],
        Vec::new(),
    );

    let ops = behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 31_402_131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    );
    (ops, modifiers)
}

fn run(current: i32) -> (Vec<RuleOp>, SkillModifiers) {
    let (ops, modifiers) = evaluate(fight(current));
    (ops.unwrap(), modifiers)
}

#[test]
fn consumes_source_counter_before_damage_and_scales_the_full_value() {
    let (ops, modifiers) = run(5_088);

    assert_eq!(modifiers.rates[0].fixed_value(), Some(12_720));
    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
                BuffSetState {
                    target_uid: 10,
                    buff_uid: 20,
                    act_info: Some(info),
                    ..
                }
            ))),
            RuleOp::BuffActInfoMarker(marker)
        ] if info[0].param == [0, 10_605]
            && marker.target_uid == 10
            && marker.buff_uid == 20
            && marker.act_id == 1147
            && marker.params == [0, 10_605]
    ));
}

#[test]
fn partial_thousand_is_scaled_and_cleared() {
    let (ops, modifiers) = run(999);

    assert_eq!(ops.len(), 2);
    assert_eq!(modifiers.rates[0].fixed_value(), Some(2_497));
}

#[test]
fn empty_counter_emits_no_state_or_rate() {
    let (ops, modifiers) = run(0);

    assert!(ops.is_empty());
    assert!(modifiers.rates.is_empty());
}

#[test]
fn missing_counter_rejects_execution() {
    let mut fight = fight(0);
    fight.attacker.as_mut().unwrap().entitys[0].buffs.clear();

    let (ops, modifiers) = evaluate(fight);

    assert!(ops.is_none());
    assert!(modifiers.rates.is_empty());
}
