use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::gauge::{GaugeCommand, GaugeOperation},
    skill::{
        behavior::classify::BehaviorSpec,
        effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::TargetRequest,
    },
};

#[test]
fn raspberry_threshold_cast_waits_for_both_round_start_losses() {
    crate::test_support::init_config();
    let buff = |uid, buff_id| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(10),
        ..Default::default()
    };
    let entity = |uid, team, buffs| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(30_000),
        attr: Some(HeroAttribute {
            hp: Some(30_000),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    1,
                    vec![
                        buff(101, 6270501),
                        buff(102, 31200184),
                        buff(103, 31200124),
                        buff(104, 31250151),
                    ],
                ),
                entity(11, 1, vec![buff(105, 31250151)]),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31200222], []);
    catalog.insert(ParsedSkillEffect {
        skill_id: 31200192,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
    });
    managers
        .execute_gauge(GaugeCommand::new(
            CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(953, "BloodPoolTag"),
            },
            crate::engine::mechanic::bloodtithe::rule::key(1),
            GaugeOperation::Enable { max: Some(100) },
        ))
        .unwrap();
    assert_eq!(
        managers.advance_rule_progress(
            10,
            102,
            DefinitionKey::new(1009, "BloodValueUseSkill"),
            6000,
            5000
        ),
        0
    );

    let result = run_group_event(
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 3,
                ..Default::default()
            },
        ),
        BattleEvent::RoundStart,
        ReactionLane::BuffActsBeforeSettlement,
        Some(&[10, 11]),
    )
    .unwrap();
    let mut sequence = Vec::new();
    for outcome in result.outcomes {
        match outcome {
            RuleOutcome::Hp(hp)
                if crate::engine::skill::buff_act::raspberry::owns_loss(hp.changes.origin) =>
            {
                sequence.push(hp.changes.target_uid);
            }
            RuleOutcome::ExPoint(_) => sequence.push(0),
            _ => {}
        }
    }
    assert_eq!(sequence, [10, 11, 0]);
}
