use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    runtime::{determinism::RoundDeterminism, drain},
    skill::{action::ActionEvent, target::TargetContext},
};

#[test]
fn heals_living_allies_once_per_completed_ultimate() {
    crate::test_support::init_config();
    let entity = |uid, hp, team_type, attack| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            attack: Some(attack),
            hp: Some(5_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut healer = entity(10, 1_000, 1, 1_000);
    healer.buffs.push(BuffInfo {
        uid: Some(30),
        buff_id: Some(31572002),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![healer, entity(11, 1_000, 1, 2_000), entity(12, 0, 1, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1_000, 2, 3_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31571131, 31570111], []);
    for (source_uid, skill_id, moxie_spent, healer_hp, gain) in [
        (10, 31571131, 5, 1_000, 1_500),
        (11, 31571131, 0, 1_000, 300),
        (11, 31570111, 5, 1_000, 0),
        (-1, 31571131, 5, 1_000, 0),
        (11, 31571131, 5, 0, 0),
    ] {
        let mut fight = fight.clone();
        fight.attacker.as_mut().unwrap().entitys[0].current_hp = Some(healer_hp);
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        drain::run_command_group(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Publish(BattleEvent::AllyAction(ActionEvent {
                source_uid,
                skill_id,
                moxie_spent,
                ..Default::default()
            }))],
        )
        .unwrap();
        assert_eq!(managers.hp.current(10), healer_hp + gain);
        assert_eq!(managers.hp.current(11), 1_000 + gain);
        assert_eq!(managers.hp.current(12), 0);
        assert_eq!(managers.hp.current(-1), 1_000);
    }
}
