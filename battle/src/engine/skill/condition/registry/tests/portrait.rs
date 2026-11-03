use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn portrait_bonus_requires_an_already_maxed_ally() {
    crate::test_support::init_config();
    for (level, raised, expected_hp) in [(5, false, 12900), (4, false, 10750), (5, true, 10750)] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3086),
                    team_type: Some(1),
                    entity_type: Some(1),
                    position: Some(1),
                    current_hp: Some(10750),
                    attr: Some(HeroAttribute {
                        hp: Some(10750),
                        attack: Some(1769),
                        ..Default::default()
                    }),
                    ex_skill_level: Some(level),
                    passive_skill: vec![113330012],
                    buffs: if raised {
                        vec![BuffInfo {
                            uid: Some(1),
                            buff_id: Some(113330011),
                            ..Default::default()
                        }]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let mut managers = BattleManagers::seeded(&fight);
        drain::run_setup_stage(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            SetupStage::RoundStartCondition,
            101,
        )
        .unwrap();
        assert_eq!(
            managers.hp.current(10),
            expected_hp,
            "level={level} raised={raised}"
        );
        assert_eq!(managers.hp.max(10), expected_hp);
        assert_eq!(
            managers.buff.has_buff_id(10, 113330012),
            expected_hp == 12900
        );
    }
    assert_eq!(
        find_key(751101, "ExSkillLevel").unwrap().role,
        ConditionRole::Predicate
    );
    assert!(find_key(751101, "SkillLevel").is_none());
    assert!(find_key(751102, "ExSkillLevel").is_none());
    assert!(parse(751101, "ExSkillLevel", &["6".into()]).is_none());
    assert!(parse(751101, "ExSkillLevel", &["5".into(), "5".into()]).is_none());
}
