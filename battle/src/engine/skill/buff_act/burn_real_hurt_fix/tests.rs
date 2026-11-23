use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

fn managers(current_hp: i32) -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(current_hp),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30940151),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn joe_halves_burn_damage_without_crossing_the_configured_hp_floor() {
    assert_eq!(adjusted_amount(&managers(3_000), 10, 1_000), 500);
    assert_eq!(adjusted_amount(&managers(1_800), 10, 1_000), 300);
}

#[test]
fn burn_without_a_real_hurt_fix_preserves_overkill_damage() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(300),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(adjusted_amount(&managers, 10, 1_000), 1_000);
}
