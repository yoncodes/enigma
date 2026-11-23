use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::manager::BattleManagers;

#[test]
fn fear_of_death_reads_charons_consternation_layers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31280114),
                    layer: Some(4),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    uid: Some(-1),
                    buff_id: Some(31280111),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.act_id() == Some(1036))
        .unwrap();

    assert_eq!(
        attribute_delta(
            &feature,
            AttrId::CriticalDef,
            &managers.buff,
            LayerScope::SourceOrOwner,
        ),
        -400
    );
    assert_eq!(
        attribute_delta(
            &feature,
            AttrId::DmgTakenReduction,
            &managers.buff,
            LayerScope::SourceOrOwner,
        ),
        -300
    );
}

fn rhiannon_managers(
    rapport_buff_id: i32,
    rapport_layers: &[i32],
    modifier_id: i32,
    target_rapport_layer: Option<i32>,
) -> BattleManagers {
    let mut target_buffs = vec![BuffInfo {
        uid: Some(-1),
        buff_id: Some(modifier_id),
        from_uid: Some(10),
        ..Default::default()
    }];
    if let Some(layer) = target_rapport_layer {
        target_buffs.push(BuffInfo {
            uid: Some(-2),
            buff_id: Some(rapport_buff_id),
            layer: Some(layer),
            from_uid: Some(-1),
            ..Default::default()
        });
    }
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: rapport_layers
                    .iter()
                    .enumerate()
                    .map(|(index, &layer)| BuffInfo {
                        uid: Some(index as i64 + 1),
                        buff_id: Some(rapport_buff_id),
                        from_uid: Some(10),
                        layer: Some(layer),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: target_buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    BattleManagers::seeded(&fight)
}

#[test]
fn rhiannon_critical_defense_reads_source_rapport_and_caps_layers() {
    crate::test_support::init_config();

    for (layers, expected) in [
        (&[][..], -150),
        (&[3][..], -240),
        (&[2, 2][..], -270),
        (&[3, 4][..], -300),
        (&[8][..], -300),
    ] {
        let managers = rhiannon_managers(31460003, layers, 31460212, None);
        let feature = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| feature.act_id() == Some(1141))
            .unwrap();

        assert_eq!(
            attribute_delta(
                &feature,
                AttrId::CriticalDef,
                &managers.buff,
                LayerScope::Source,
            ),
            expected
        );
        assert_eq!(
            managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
            expected
        );
    }

    let managers = rhiannon_managers(31460004, &[8], 31460214, None);
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -410
    );

    let bonus = rhiannon_managers(31460003, &[3, 4], 31460222, None);
    assert_eq!(bonus.persistent_attribute_delta(-1, AttrId::DmgBonus), 300);

    let managers = rhiannon_managers(31460003, &[3], 31460212, Some(8));
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -240
    );
}
