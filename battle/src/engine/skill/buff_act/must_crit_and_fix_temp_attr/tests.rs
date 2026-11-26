use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroExAttribute};

use super::*;

#[test]
fn extra_action_uses_the_configured_critical_attributes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(31050186),
                    from_uid: Some(20),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(400),
            cri_dmg: Some(1750),
            ..Default::default()
        },
    );
    let features = managers.buff.active_features(&managers.hp);
    let configured = features
        .iter()
        .find(|active| {
            super::super::is_kind(active, BuffActKind::MustCritAndFixTempAttr)
                && active.values.get(1) == Some(&205)
        })
        .unwrap();

    assert_eq!(
        attribute_delta(
            configured,
            AttrId::DmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
        ),
        300
    );
    assert_eq!(
        attribute_delta(
            configured,
            AttrId::GenesisDmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
        ),
        0
    );
    assert!(!forces_critical(&features, 10, false));
    assert!(forces_critical(&features, 10, true));
    assert!(!active_forces_critical(10, &managers, false));
    assert!(active_forces_critical(10, &managers, true));
    assert_eq!(
        super::super::calculated_attack_attribute_delta_for_skill(
            configured,
            AttrId::DmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            super::super::AttackAttributeScope {
                is_big_skill: false,
                extra_action: false,
                performs_extra_action: true,
                source_performs_extra_action: true,
            },
        ),
        300
    );
    assert!(forces_critical(&features, 10, true));

    let mut malformed = configured.clone();
    malformed.values = vec![860, 205, 999, 203];
    assert_eq!(
        attribute_delta(
            &malformed,
            AttrId::DmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
        ),
        0
    );
    assert!(!forces_critical(&[malformed], 10, true));
}
