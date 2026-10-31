use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{manager::BattleManagers, skill::target::TargetPool};

use super::outcome::{BattleOutcome, battle_outcome, defenders_defeated};

#[test]
fn live_reserve_keeps_the_battle_open_for_promotion() {
    crate::test_support::init_config();
    let entity = |uid, hp, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 0, 1)],
            sub_entitys: vec![entity(-2, 100, -1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    assert!(!defenders_defeated(&pool, &managers));
    assert_eq!(
        battle_outcome(&fight, &pool, &managers),
        BattleOutcome::Unfinished
    );
    assert_eq!(managers.promote_reserves(&mut fight).len(), 1);
    assert_eq!(fight.defender.unwrap().entitys[0].uid, Some(-2));
}
