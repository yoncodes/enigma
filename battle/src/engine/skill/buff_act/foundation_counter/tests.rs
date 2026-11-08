use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

fn managers(current: i32) -> BattleManagers {
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(5_000),
                attr: Some(HeroAttribute {
                    hp: Some(5_000),
                    attack: Some(2_121),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31_400_003),
                    from_uid: Some(10),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1147),
                        param: vec![current, 10_605],
                        str_param: Some(String::new()),
                    }],
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
fn accumulates_prevented_damage_and_caps_at_the_snapshot() {
    crate::test_support::init_config();

    let update = update(&managers(10_000), 10, 1_696).unwrap().unwrap();

    assert_eq!(update.marker.params, [10_605, 10_605]);
}

#[test]
fn missing_counter_carrier_fails_loudly() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight::default());

    assert!(matches!(
        update(&managers, 10, 1_696),
        Err(CounterError::MissingCarrier)
    ));
}

#[test]
fn malformed_counter_state_fails_loudly() {
    crate::test_support::init_config();

    assert!(matches!(
        update(&managers(10_606), 10, 1_696),
        Err(CounterError::InvalidState)
    ));
}
