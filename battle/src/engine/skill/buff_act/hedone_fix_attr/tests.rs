use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

fn managers(buff_id: i32) -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(buff_id),
                    from_uid: Some(10),
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
fn molten_scar_adds_only_the_captured_conduit_damage_bonus() {
    let managers = managers(31560122);

    assert_eq!(
        incoming_attributes(&managers, -1, true),
        vec![(AttrId::DmgBonus, 80)]
    );
    assert!(incoming_attributes(&managers, -1, false).is_empty());
    assert!(incoming_attributes(&managers, 10, true).is_empty());
}

#[test]
fn test_carrier_keeps_each_configured_damage_lane() {
    assert_eq!(
        incoming_attributes(&managers(115390006), -1, true),
        vec![(AttrId::DmgBonus, 150), (AttrId::DmgTakenReduction, -150),]
    );
}

#[test]
fn supports_only_the_proven_pair_shape() {
    assert!(supports(&[205, 0, 205, 80]));
    assert!(supports(&[205, 150, 205, -150]));
    assert!(!supports(&[205, 0, 205, 0]));
    assert!(!supports(&[206, 0, 205, 80]));
    assert!(!supports(&[205, 0, 205]));
}
