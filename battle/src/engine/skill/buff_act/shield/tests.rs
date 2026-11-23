use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn shield_rate_scales_from_the_sources_tracked_layers() {
    crate::test_support::init_config();
    let mut buffs = BuffManager::default();
    buffs.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(4150001),
                    layer: Some(6),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(
        configured_attr_rate(30940121, 10, &buffs),
        Some((AttrId::Attack, 1180))
    );
    buffs.set_catalog(crate::catalog::BattleCatalog::new(
        crate::test_support::game_data(),
    ));
    assert_eq!(
        configured_attr_rate(30940121, 10, &buffs),
        Some((AttrId::Attack, 1180))
    );
}

#[test]
fn cumulative_shield_mode_exposes_the_carrier_tier_rate() {
    crate::test_support::init_config();
    let mut buffs = BuffManager::default();
    buffs.set_catalog(crate::catalog::BattleCatalog::new(
        crate::test_support::game_data(),
    ));

    assert_eq!(
        cumulative_attr_rate(116385674, &buffs),
        Some((AttrId::Hp, 400))
    );
    assert_eq!(cumulative_attr_rate(31270002, &buffs), None);
}
