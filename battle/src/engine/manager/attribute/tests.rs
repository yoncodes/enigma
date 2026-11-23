use super::*;

#[test]
fn account_attributes_override_config_baseline() {
    let mut manager = AttributeManager::default();
    manager.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(385),
            cri_dmg: Some(1480),
            ..Default::default()
        },
    );

    assert_eq!(manager.get(10, AttrId::CriticalRate), 385);
    assert_eq!(manager.get(10, AttrId::CriticalDmg), 1480);
}

#[test]
fn account_action_attributes_keep_their_distinct_lanes() {
    let mut manager = AttributeManager::default();
    manager.override_sp(
        10,
        &HeroSpAttribute {
            rebound_dmg: Some(30),
            extra_dmg: Some(60),
            reuse_dmg: Some(100),
            device_skill_rate: Some(140),
            defense_ignore: Some(80),
            normal_skill_rate: Some(120),
            ..Default::default()
        },
    );

    assert_eq!(manager.get(10, AttrId::ReboundDmg), 30);
    assert_eq!(manager.get(10, AttrId::ExtraDmg), 60);
    assert_eq!(manager.get(10, AttrId::ReuseDmg), 100);
    assert_eq!(manager.get(10, AttrId::ConduitMight), 140);
    assert_eq!(manager.get(10, AttrId::Penetration), 80);
    assert_eq!(manager.get(10, AttrId::IncantationMight), 120);
}

#[test]
fn emitter_inherits_the_team_average_combat_attributes() {
    crate::test_support::init_config();
    let entity = |uid, model_id| sonettobuf::FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        level: Some(180),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![entity(10, 3108), entity(11, 3113)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = AttributeManager::default();

    manager.seed(&fight);
    manager.override_ex(
        10,
        &HeroExAttribute {
            add_dmg: Some(100),
            cri_dmg: Some(1400),
            ..Default::default()
        },
    );
    manager.override_ex(
        11,
        &HeroExAttribute {
            add_dmg: Some(300),
            cri_dmg: Some(1600),
            ..Default::default()
        },
    );
    manager.sync_emitter_average(&fight);

    assert_eq!(
        manager.get(crate::engine::manager::emitter::UID, AttrId::DmgBonus),
        200
    );
    assert_eq!(
        manager.get(crate::engine::manager::emitter::UID, AttrId::CriticalDmg),
        1500
    );
}
