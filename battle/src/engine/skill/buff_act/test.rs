use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    damage::DamageFormula,
    event::{
        kind::EventKind,
        payload::{BattleEvent, BuffChangeEvent},
    },
    manager::{
        BattleManagers,
        buff::BuffManager,
        eureka::EurekaCommand,
        ex_point::{ExPointCommand, ExPointMaxWire},
        hp::{HpCommand, HpManager},
    },
    skill::{
        action::SkillInvocation,
        rule::output::{BattleCommand, RuleOp},
    },
};

fn feature(act_type: &str, values: Vec<i32>, owner_uid: i64) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: owner_uid,
        buff_uid: 2,
        buff_id: 1,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: act_type.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    }
}

fn transaction_event(buff_id: i32, before_amount: i32, after_amount: i32) -> BattleEvent {
    let change = BuffChangeEvent {
        source_uid: 10,
        target_uid: 10,
        buff_uid: 2,
        buff_id,
        before_amount,
        after_amount,
        act_id: 0,
        act_value: 0,
    };
    if after_amount == 0 {
        BattleEvent::BuffRemoved(change)
    } else if before_amount == 0 {
        BattleEvent::BuffAdded(change)
    } else {
        BattleEvent::BuffChanged(change)
    }
}

fn managers_with_buff(buff_id: i32, amount: i32) -> BattleManagers {
    let buffs = if amount > 0 {
        vec![BuffInfo {
            uid: Some(2),
            buff_id: Some(buff_id),
            from_uid: Some(10),
            layer: Some(amount),
            count: Some(amount),
            ..Default::default()
        }]
    } else {
        Vec::new()
    };
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn max_hp_delta(event: BattleEvent) -> i32 {
    let after_amount = match &event {
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => change.after_amount,
        _ => unreachable!(),
    };
    transaction_rule_ops(&managers_with_buff(30_800_151, after_amount), &event)
        .into_iter()
        .find_map(|(_, op)| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(change))) => Some(change.delta),
            _ => None,
        })
        .expect("the HP attribute feature should emit a max-HP transaction")
}

fn power_max_delta(event: BattleEvent) -> i32 {
    let after_amount = match &event {
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => change.after_amount,
        _ => unreachable!(),
    };
    transaction_rule_ops(&managers_with_buff(31_050_147, after_amount), &event)
        .into_iter()
        .find_map(|(_, op)| match op {
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::ChangeMax { delta, .. })) => {
                Some(delta)
            }
            _ => None,
        })
        .expect("the power feature should emit a max-power transaction")
}

#[test]
fn grant_features_keep_config_order() {
    crate::test_support::init_config();

    let values = BuffManager::configured_features(70_015)
        .into_iter()
        .map(|feature| feature.values)
        .collect::<Vec<_>>();

    assert_eq!(values, vec![vec![100, 102, 100], vec![100, 206, 150]]);
}

#[test]
fn buff_act_type_can_own_its_runtime_event() {
    assert_eq!(
        registry::runtime_event(731, "CastChannel", 0),
        Some(EventKind::RoundStart)
    );
    assert_eq!(
        registry::runtime_event(770, "InjuryBank", 0),
        Some(EventKind::HpLost)
    );
    assert_eq!(registry::runtime_event(719, "PowerMaxAdd", 103), None);
    assert_eq!(
        registry::runtime_event(503, "AddToTarget", 209),
        Some(EventKind::BeAttacked)
    );
}

#[test]
fn stacked_max_hp_attr_uses_the_buff_amount_delta() {
    crate::test_support::init_config();

    assert_eq!(max_hp_delta(transaction_event(30_800_151, 0, 1)), 50);
    assert_eq!(max_hp_delta(transaction_event(30_800_151, 1, 2)), 50);
    assert_eq!(max_hp_delta(transaction_event(30_800_151, 2, 0)), -100);
}

#[test]
fn power_max_transactions_use_the_buff_amount_delta() {
    crate::test_support::init_config();

    assert_eq!(power_max_delta(transaction_event(31_050_147, 0, 1)), 2);
    assert_eq!(power_max_delta(transaction_event(31_050_147, 1, 3)), 4);
    assert_eq!(power_max_delta(transaction_event(31_050_147, 3, 0)), -6);
}

#[test]
fn special_moxie_cap_uses_the_configured_cap_and_ultimate_cost() {
    crate::test_support::init_config();

    let managers = managers_with_buff(31_000_161, 1);
    let command = transaction_rule_ops(&managers, &transaction_event(31_000_161, 0, 1))
        .into_iter()
        .find_map(|(_, op)| match op {
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::ChangeMax(change))) => {
                Some(change)
            }
            _ => None,
        })
        .expect("the special max-moxie feature should emit a manager command");

    assert_eq!(command.delta, 7);
    assert_eq!(
        command.wire,
        ExPointMaxWire::Special {
            max_add: 7,
            ultimate_cost_offset: 3,
        }
    );
    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.ultimate_cost_offset(&managers, 10),
        3
    );
}

#[test]
fn destination_probe_uses_the_same_exact_rule_op_path_as_runtime() {
    let args = [123_456, 1];
    let ops = registry::linked_rule_ops(10, 759, "UseSkillToEnemy", &args)
        .expect("the exact registered route should emit a skill invocation");

    assert!(registry::has_destination(759, "UseSkillToEnemy", &args));
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Skill(SkillInvocation { plan, .. })]
            if plan.source_uid == 10 && plan.skill_id == 123_456
    ));
    assert!(!registry::has_destination(
        759,
        "MonitorContinueChannel",
        &args
    ));
}

#[test]
fn exact_registry_routes_attack_replacements_without_using_additional_damage_for_direct_hits() {
    let mut hp = HpManager::default();
    hp.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(20_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let hp_skill = feature(
        "AttrOnlyCalDamageHpReplaceAttackCalSkillDamage",
        vec![1022, 200, 0],
        10,
    );
    assert_eq!(
        direct_attack_replacement_rule(&hp_skill, &hp),
        Some(AttackReplacement {
            replaced_attr: AttrId::Attack,
            source_attr: AttrId::Hp,
            amount: 4_000,
            formula: DamageFormula::HpSkillDamage,
        })
    );

    let additional = feature(
        "AttrOnlyCalDamageReplaceAttrADCreator",
        vec![1005, 102, 101, 200],
        10,
    );
    assert_eq!(
        attack_replacement_rule(&additional, &hp).map(|replacement| replacement.formula),
        Some(DamageFormula::AdditionalDamage)
    );
    assert_eq!(direct_attack_replacement_rule(&additional, &hp), None);

    let direct = feature(
        "AttrOnlyCalDamageReplaceAttr",
        vec![1007, 102, 101, 200],
        10,
    );
    assert_eq!(
        direct_attack_replacement_rule(&direct, &hp).map(|replacement| replacement.formula),
        Some(DamageFormula::AttributeReplacement)
    );
}
