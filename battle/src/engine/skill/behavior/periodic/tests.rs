use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::classify::BehaviorSpec,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn poison_settlement_repeats_scaled_damage_without_consuming_duration() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let buff_uids = [31420000, 31040005].map(|buff_id| {
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: -1,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap()
            .change
            .added
            .unwrap()
            .buff
            .uid
            .unwrap()
    });
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60083, "SettleDotAndCostDotDuration"),
        vec![2, 1, 4000],
        vec!["2".into(), "1".into(), "4000".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert_eq!(ops.len(), 4);
    assert_eq!(
        ops.iter()
            .map(|op| match op {
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))) => loss.amount,
                _ => 0,
            })
            .collect::<Vec<_>>(),
        [1_600, 800, 1_600, 800]
    );
    assert!(ops.iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss)))
            if loss.config_effect == 60083
                && loss.hurt.as_ref().and_then(|hurt| hurt.display_amount) == Some(loss.amount)
    )));
    for buff_uid in buff_uids {
        assert_eq!(
            managers.buff.snapshot(-1, buff_uid).unwrap().duration,
            Some(2)
        );
    }
}
