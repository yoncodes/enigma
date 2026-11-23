use crate::engine::{
    event::kind::EventKind,
    manager::BattleManagers,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::SkillPhase,
        action::{SkillInvocation, SkillRequest, SkillTarget},
        condition::registry::{self, ConditionRole},
        effect::SkillEffectCatalog,
        rule::{SetupStage, output::RuleOp},
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn hedone_opening_rank_keeps_its_after_round_start_owner() {
    assert_eq!(
        registry::parse(789105, "Rank", &["2".into()]),
        Some(crate::engine::skill::condition::parse::ParsedConditionKind::RankAtLeast(2))
    );
    assert_eq!(
        registry::find_key(789105, "Rank").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::AfterRoundStart,
            priority: 0,
        })
    );
}

#[test]
fn hedone_ultimate_rank_keeps_its_after_hit_owner() {
    assert_eq!(
        registry::parse(789210, "Rank", &["2".into()]),
        Some(crate::engine::skill::condition::parse::ParsedConditionKind::RankAtLeast(2))
    );
    assert_eq!(
        registry::find_key(789210, "Rank").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn unlock_reward_uses_insight() {
    crate::test_support::init_config();
    for (level, reward) in [(30, 0), (31, 3), (150, 3)] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3147),
                    entity_type: Some(1),
                    team_type: Some(1),
                    level: Some(level),
                    ex_point: Some(0),
                    current_hp: Some(1000),
                    attr: Some(HeroAttribute {
                        hp: Some(1000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31470163], []);
        let mut invocation = SkillInvocation::from(SkillRequest {
            source_uid: 10,
            skill_id: 31470163,
        });
        invocation.target = SkillTarget::LogicRule(103);
        drain::run_command_group(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        assert_eq!(managers.ex_point.get(10), reward, "level {level}");
        assert!(
            managers
                .buff
                .active_for(10)
                .any(|buff| buff.buff_id == Some(31470011))
        );
    }
    assert!(registry::parse(789208, "Rank", &["0".into()]).is_none());
    assert!(registry::parse(789208, "Rank", &["2".into(), "3".into()]).is_none());
    assert!(registry::find_key(789002, "Rank").is_none());
}

#[test]
fn hedone_opening_rank_accepts_higher_rank() {
    crate::test_support::init_config();
    let matches = |level| {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3156),
                    entity_type: Some(1),
                    team_type: Some(1),
                    level: Some(level),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);
        super::matches(2, &[10], &managers, &pool)
    };

    assert!(matches(31));
    assert!(matches(121));
}
