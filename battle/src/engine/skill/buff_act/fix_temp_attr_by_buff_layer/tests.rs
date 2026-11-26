use super::*;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn field_uses_all_tracked_stacks_on_its_source() {
    crate::test_support::init_config();
    let mut buffs = BuffManager::default();
    buffs.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31050111),
                            layer: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31050111),
                            layer: Some(6),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    buffs: vec![BuffInfo {
                        buff_id: Some(31050145),
                        uid: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let feature = ActiveBuffFeature {
        owner_uid: 20,
        source_uid: 10,
        buff_uid: 1,
        buff_id: 31050145,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "FixTempAttrByBuffLayer".into(),
        effect_time: 203,
        effect_condition: 0,
        raw: "861#31050111#1#205#20".into(),
        values: vec![861, 31050111, 1, 205, 20],
    };

    assert_eq!(buffs.grant_value(1, 861), None);
    assert_eq!(attribute_delta(&feature, AttrId::DmgBonus, &buffs), 320);
    let hp = crate::engine::manager::hp::HpManager::default();
    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &feature,
            AttrId::DmgBonus,
            &buffs,
            &hp,
            false,
            false,
        ),
        0
    );
    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &feature,
            AttrId::DmgBonus,
            &buffs,
            &hp,
            false,
            true,
        ),
        320
    );
    let attributes = crate::engine::manager::attribute::AttributeManager::default();
    assert_eq!(
        super::super::calculated_attack_attribute_delta_for_skill(
            &feature,
            AttrId::DmgBonus,
            &attributes,
            &buffs,
            &hp,
            super::super::AttackAttributeScope {
                is_big_skill: false,
                extra_action: false,
                performs_extra_action: true,
                source_performs_extra_action: true,
            },
        ),
        320
    );
    assert_eq!(
        attribute_delta(&feature, AttrId::GenesisDmgBonus, &buffs),
        0
    );
    assert!(supports(&[31050111, 1, AttrId::DmgBonus.id(), 25]));
    assert!(supports(&[31050111, 1, AttrId::GenesisDmgBonus.id(), 25]));
    assert!(!supports(&[31050111, 0, AttrId::DmgBonus.id(), 25]));
    assert!(!supports(&[31050111, 1, AttrId::Attack.id(), 25]));
}
