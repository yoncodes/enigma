use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, rule_ops},
        target::{TargetContext, TargetPool},
    },
};

fn ops(current_hp: i32, mode: i32, is_crit: bool) -> Vec<RuleOp> {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(current_hp),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_200),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(100, 10, [is_crit]);
    let mut behavior = ParsedBehavior::new(
        60179,
        "HealOrAddBuffByHp",
        vec![500, mode, 102, 800, 30470151],
    );
    behavior.config_effect = 60179;
    rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 100,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut SkillModifiers::default(),
            target: &mut TargetContext::default(),
        },
        &behavior,
    )
    .unwrap()
}

#[test]
fn hp_branch_heals_at_the_threshold_from_the_configured_owner() {
    assert!(matches!(
        ops(500, 0, false).as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal)))]
            if heal.amount == 800 && heal.kind == HpHealKind::Normal
    ));
    assert!(matches!(
        ops(500, 1, false).as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal)))]
            if heal.amount == 960
    ));
}

#[test]
fn hp_branch_preserves_the_captured_heal_crit_kind() {
    assert!(matches!(
        ops(500, 0, true).as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal)))]
            if heal.kind == HpHealKind::Critical && heal.config_effect == 60179
    ));
}

#[test]
fn hp_branch_grants_the_configured_buff_above_the_threshold() {
    assert!(matches!(
        ops(501, 0, false).as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))]
            if grant.buff_id == 30470151
                && grant.source_uid == 10
                && grant.target_uid == 11
    ));
}

#[test]
fn hp_branch_grant_uses_manager_owned_replacement() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(501),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(6240515),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 100,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut SkillModifiers::default(),
            target: &mut TargetContext::default(),
        },
        &ParsedBehavior::new(60179, "HealOrAddBuffByHp", vec![500, 0, 99, 250, 6240515]),
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
        panic!("expected buff grant");
    };
    let changes = managers.execute_buff(command.clone()).unwrap();

    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(20));
    assert_eq!(
        changes.change.added.as_ref().unwrap().buff.buff_id,
        Some(6240515)
    );
}
