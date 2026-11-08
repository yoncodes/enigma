#[test]
fn damage_modifiers_use_only_captured_add_and_refresh_markers() {
    for (act_id, type_name, marker) in [
        (
            520,
            "RealHarmFix",
            sonettobuf::effect_type_enum::EffectType::Realharmfix as i32,
        ),
        (
            740,
            "AttrOnlyCalDamageInExtra",
            sonettobuf::effect_type_enum::EffectType::None as i32,
        ),
    ] {
        let wire = super::super::super::wire::find(act_id, type_name).unwrap();
        assert_eq!(
            wire.markers(super::super::super::wire::WirePhase::Add),
            &[marker]
        );
        assert!(
            wire.markers(super::super::super::wire::WirePhase::Static)
                .is_empty()
        );
        assert_eq!(
            wire.markers(super::super::super::wire::WirePhase::Refresh),
            &[marker]
        );
    }
}

#[test]
fn basic_incantation_defense_is_attack_local() {
    crate::test_support::init_config();
    let definition = super::find(111, "Attr").unwrap();
    assert_eq!(
        definition.kind,
        super::BuffActKind::AttrOnlyCalDamageBasicAttacked
    );
    assert!(
        crate::engine::skill::buff_act::attr_only_cal_damage_attack::supports_basic_attacked(&[
            crate::engine::entity::attr::AttrId::CriticalResistRate as i32,
            100
        ],)
    );
    assert!(
        !crate::engine::skill::buff_act::attr_only_cal_damage_attack::supports_basic_attacked(&[
            crate::engine::entity::attr::AttrId::Attack as i32,
            100
        ],)
    );
    assert!(definition.transaction.events.is_empty());
    let wire = super::super::super::wire::find(111, "Attr").unwrap();
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Attr as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Refresh),
        &[sonettobuf::effect_type_enum::EffectType::Attr as i32]
    );

    let managers = crate::engine::manager::BattleManagers::seeded(&sonettobuf::Fight {
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109390001),
                    from_uid: Some(10),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    use crate::engine::{entity::attr::AttrId, skill::target::EntityDamageType};
    assert_eq!(
        managers
            .buff
            .attribute_delta(10, AttrId::CriticalResistRate),
        0
    );
    assert_eq!(
        super::super::super::incoming_target_attack_attribute_delta(
            &managers,
            10,
            EntityDamageType::Mental,
            false,
            AttrId::CriticalResistRate,
        ),
        300
    );
    assert_eq!(
        super::super::super::incoming_target_attack_attribute_delta(
            &managers,
            10,
            EntityDamageType::Mental,
            false,
            AttrId::DmgTakenReduction,
        ),
        300
    );
    assert_eq!(
        super::super::super::incoming_target_attack_attribute_delta(
            &managers,
            10,
            EntityDamageType::Mental,
            true,
            AttrId::DmgTakenReduction,
        ),
        0
    );
}

#[test]
fn one_shot_basic_incantation_defense_consumes_after_trigger() {
    crate::test_support::init_config();
    use crate::engine::{entity::attr::AttrId, skill::target::EntityDamageType};
    let mut managers = crate::engine::manager::BattleManagers::seeded(&sonettobuf::Fight {
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(7011),
                    from_uid: Some(10),
                    layer: Some(1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(
        super::super::super::incoming_target_attack_attribute_delta(
            &managers,
            10,
            EntityDamageType::Mental,
            false,
            AttrId::CriticalResistRate,
        ),
        -500
    );
    assert!(
        super::super::super::be_attacked_consumption_rule_ops(
            &managers,
            10,
            &[EntityDamageType::Mental],
            true,
        )
        .is_empty()
    );
    let ops = super::super::super::be_attacked_consumption_rule_ops(
        &managers,
        10,
        &[EntityDamageType::Mental],
        false,
    );
    let [
        (
            _,
            crate::engine::skill::rule::output::RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Buff(command),
            ),
        ),
    ] = ops.as_slice()
    else {
        panic!("expected one effect-count consumption");
    };
    managers.execute_buff(command.clone()).unwrap();
    assert_eq!(
        super::super::super::incoming_target_attack_attribute_delta(
            &managers,
            10,
            EntityDamageType::Mental,
            false,
            AttrId::CriticalResistRate,
        ),
        0
    );
}
