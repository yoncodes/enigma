use super::*;
use crate::engine::{
    manager::gauge::{GaugeCommand, GaugeOperation},
    skill::rule::{DefinitionKey, RuleDomain},
};
use sonettobuf::Fight;

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60107, "ProgressChange"),
};

fn gain(team: i32) -> Gain {
    Gain {
        origin: ORIGIN,
        key: crate::engine::mechanic::bloodtithe::rule::key(team),
        rule_skill: 8300201,
        per_point: 1,
    }
}

fn apply_blood(managers: &mut BattleManagers, gain: Gain, deltas: &[i32]) {
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            gain.origin,
            gain.key,
            GaugeOperation::Enable { max: Some(58) },
        ))
        .unwrap();
    for &delta in deltas {
        managers
            .gauge
            .execute_command(GaugeCommand::new(
                gain.origin,
                gain.key,
                GaugeOperation::ChangeValue { delta },
            ))
            .unwrap();
    }
}

#[test]
fn bloodtithe_progress_claims_gains_once_across_phases_without_spending_blood() {
    let mut managers = BattleManagers::seeded(&Fight {
        progress: Some(76),
        progress_max: Some(100),
        ..Default::default()
    });
    let attacker = gain(1);
    let defender = gain(2);
    apply_blood(&mut managers, attacker, &[3, 16, 1, 1, -4]);
    apply_blood(&mut managers, defender, &[5]);

    assert_eq!(settle(&mut managers, attacker).unwrap().unwrap().delta, 21);
    assert_eq!(pending(&managers, attacker), 0);
    assert_eq!(pending(&managers, defender), 5);
    assert_eq!(managers.gauge.get(attacker.key).unwrap().current, 17);
    assert!(settle(&mut managers, attacker).unwrap().is_none());
    assert_eq!(settle(&mut managers, defender).unwrap().unwrap().delta, 3);
    assert_eq!(pending(&managers, defender), 0);
    assert_eq!(managers.gauge.get(defender.key).unwrap().current, 5);
}

#[test]
fn failed_scalar_settlement_keeps_the_bloodtithe_gain_pending() {
    let mut managers = BattleManagers::seeded(&Fight::default());
    let gain = gain(1);
    apply_blood(&mut managers, gain, &[6]);

    assert!(settle(&mut managers, gain).is_err());
    assert_eq!(pending(&managers, gain), 6);
}
#[test]
fn incoming_bloodtithe_progress_settles_before_attacked_ally_reactions() {
    use crate::engine::{
        runtime::{determinism::RoundDeterminism, drain, executor::RuleOutcome},
        skill::{
            action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
            effect::SkillEffectCatalog,
            target::{TargetContext, TargetPool},
        },
    };
    use sonettobuf::{FightEntityInfo, FightTeam, HeroAttribute};

    let progress = gain(1);
    let attack = 40110411;
    let ally_heal = 31250144;
    let ally_heal_marker = 31250191;
    let entity = |uid, passives| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(800),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            attack: Some(200),
            ..Default::default()
        }),
        passive_skill: passives,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, vec![progress.rule_skill]),
                entity(11, vec![ally_heal]),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, Vec::new())],
            ..Default::default()
        }),
        progress: Some(0),
        progress_max: Some(100),
        ..Default::default()
    };
    let db = crate::test_support::game_data();
    let catalog = SkillEffectCatalog::from_roots(db, [attack, progress.rule_skill, ally_heal], []);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    apply_blood(&mut managers, progress, &[1]);

    let invocation = SkillInvocation {
        target: SkillTarget::Explicit(10),
        mode: SkillExecutionMode::Active,
        ..SkillRequest {
            source_uid: -1,
            skill_id: attack,
        }
        .into()
    };
    let result = drain::run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        [],
        invocation,
    )
    .unwrap();
    let progress_index = result
        .outcomes
        .iter()
        .position(|outcome| matches!(outcome, RuleOutcome::LegacyProgressGain(change) if change.delta > 0))
        .unwrap();
    let marker_index = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Buff(change)
                    if change.change.added.as_ref().is_some_and(|entry| entry.buff.buff_id == Some(ally_heal_marker))
            )
        })
        .unwrap();
    assert!(progress_index < marker_index);
}
