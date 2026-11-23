use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    mechanic::nuo_di_ka::NuoDiKaCommand,
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::classify::BehaviorSpec,
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn configured_channel_points_drive_random_mass_damage_and_heal() {
    crate::test_support::init_config();
    let entity = |uid, team_type, hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        team_type: Some(team_type),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(1_000),
            defense: Some(0),
            mdefense: Some(0),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 20_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 20_000), entity(-2, 2, 20_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .nuo_di_ka
        .execute(NuoDiKaCommand::Set {
            owner_uid: 10,
            points: 3,
            bloodtithe_consumed: 6,
            max_points: 30,
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60209, "NuoDiKaDamage"),
        vec![31200135, 1000, 31200132, 1000, 100],
        Vec::new(),
    );

    let ops = emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31200173,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
        Some(crate::engine::skill::effect::catalog::global()),
    )
    .unwrap();

    assert_eq!(ops.len(), 12);
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op, RuleOp::NuoDiKaHit(_)))
            .count(),
        5
    );
    assert!(matches!(
        &ops[1],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            buff_id: 31200135,
            ..
        })))
    ));
    assert!(matches!(
        &ops[5],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            selector: BuffRemoveSelector::ExactId(31200135),
            ..
        })))
    ));
    assert!(matches!(
        &ops[6],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            buff_id: 31200132,
            ..
        })))
    ));
    let RuleOp::Command(BattleCommand::HpBatch(commands)) = &ops[10] else {
        panic!("expected one aggregated HP batch");
    };
    assert_eq!(commands.len(), 2);
    assert_eq!(
        commands
            .iter()
            .map(|command| match command {
                HpCommand::Damage(damage) => damage.amount,
                _ => 0,
            })
            .sum::<i32>(),
        41_000
    );
    assert!(matches!(
        ops.last(),
        Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
            HpHeal { amount: 6_000, .. }
        ))))
    ));
}

#[test]
fn descriptor_reports_single_and_mass_buffs() {
    let behavior = ParsedBehavior::new(60209, "NuoDiKaDamage", vec![101, 100, 102, 200, 50]);

    assert_eq!(references(&behavior).buffs, vec![101, 102]);
}
