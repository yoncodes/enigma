use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::fight;
use crate::dungeon::BuiltFight;

fn tables() -> &'static config::GameDB {
    crate::test_support::init_config();
    config::configs::get()
}

#[test]
fn boss_five_setup_is_derived_from_the_captured_config_chain() {
    let tables = tables();
    let plan = tables
        .tower_talent_plan
        .iter()
        .find(|plan| plan.boss_id == 5 && plan.plan_id == 502)
        .unwrap();
    let talents = fight::system_plan_talents(tables, 5, 10, &plan.talent_ids);
    let mut built = BuiltFight {
        fight: Fight {
            attacker: Some(FightTeam {
                entitys: (1..=4)
                    .map(|career| FightEntityInfo {
                        uid: Some(career as i64),
                        level: Some(180),
                        career: Some(career),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        },
        ex_attributes: vec![],
        sp_attributes: vec![],
        battle_rule_skills: vec![],
    };

    fight::apply_assist_boss(tables, 401_299_742, 5, 10, &talents, &mut built).unwrap();

    let team = built.fight.attacker.unwrap();
    let boss = team.assist_boss.unwrap();
    assert_eq!(talents.len(), 15);
    assert_eq!(boss.uid, Some(-1));
    assert_eq!(boss.attr.unwrap().attack, Some(2380));
    assert_eq!(boss.power_infos[0].power_id, Some(4));
    assert_eq!(boss.passive_skill.len(), 14);
    assert_eq!(
        team.assist_boss_info.unwrap().skills[0].skill_id,
        Some(1251001)
    );
    assert!(team.entitys.iter().all(|hero| {
        [1252007, 1252008, 1252009, 123900605, 1259001, 1252001]
            .iter()
            .all(|skill| hero.passive_skill.contains(skill))
    }));
}

#[test]
fn tower_extra_rules_keep_virtual_and_entity_ownership_separate() {
    let tables = tables();
    let mut built = BuiltFight {
        fight: Fight {
            attacker: Some(FightTeam {
                entitys: (1..=4)
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        level: Some(1),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
        ex_attributes: vec![],
        sp_attributes: vec![],
        battle_rule_skills: vec![],
    };

    fight::apply_assist_boss(tables, 1, 1, 5, &[101], &mut built).unwrap();

    assert_eq!(
        built.battle_rule_skills,
        vec![crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
            skill_id: 370002010,
        }]
    );
    assert!(
        !built
            .fight
            .attacker
            .unwrap()
            .assist_boss
            .unwrap()
            .passive_skill
            .contains(&370002010)
    );
    assert!(
        built.fight.defender.unwrap().entitys[0]
            .passive_skill
            .contains(&370001020)
    );
}
