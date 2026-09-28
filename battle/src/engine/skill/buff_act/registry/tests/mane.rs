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
