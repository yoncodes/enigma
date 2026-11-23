use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn keeps_its_configured_attack_attribute() {
    assert_eq!(
        layer_attribute_delta(&[201, 300, 0], 2, AttrId::CriticalRate),
        300
    );
}

#[test]
fn applies_each_attribute_from_the_tracked_layer_count() {
    assert_eq!(
        layer_attribute_delta(&[104, -100, -20, 206, -100, -20], 3, AttrId::MentalDef),
        -140
    );
    assert_eq!(
        layer_attribute_delta(
            &[104, -100, -20, 206, -100, -20],
            3,
            AttrId::DmgTakenReduction,
        ),
        -140
    );
}

#[test]
fn validates_a_tracked_buff_and_attribute_triples() {
    assert!(supports(&[31260151, 201, 300, 0]));
    assert!(supports(&[31130122, 104, -100, -20, 206, -100, -20]));
    assert!(!supports(&[0, 201, 600, 0]));
    assert!(!supports(&[31260151]));
    assert!(!supports(&[31260151, 999, 300, 0]));
    assert!(!supports(&[31260151, 201, 300]));
}

#[test]
fn owner_delta_uses_owned_catalog_with_global_compatibility() {
    crate::test_support::init_config();
    let mut buffs = BuffManager::default();
    buffs.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(31130122),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31130124),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(owner_attribute_delta(&buffs, 1, AttrId::MentalDef), -140);
    buffs.set_catalog(crate::catalog::BattleCatalog::new(
        crate::test_support::game_data(),
    ));
    assert_eq!(owner_attribute_delta(&buffs, 1, AttrId::MentalDef), -140);
}
