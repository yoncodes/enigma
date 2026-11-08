use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot};

#[test]
fn conduit_energy_selects_a_living_enemy_while_ordinary_source_skills_select_the_source() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(60291, "AddDevicePower", vec![1, 1]),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_logic_target(100, super::super::request::SOURCE_TARGET_CODE);
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_logic_target(200, super::super::request::SOURCE_TARGET_CODE);
    let request = TargetRequest {
        code: super::super::request::SOURCE_TARGET_CODE,
        raw: Vec::new(),
    };

    assert_eq!(
        CardSelectionResolver::candidates(
            &request,
            100,
            10,
            &pool,
            &catalog,
            &RoundDeterminism::default(),
            None,
            TargetContext::default(),
        ),
        [-1]
    );
    assert_eq!(
        CardSelectionResolver::candidates(
            &request,
            200,
            10,
            &pool,
            &catalog,
            &RoundDeterminism::default(),
            None,
            TargetContext::default(),
        ),
        [10]
    );
}
