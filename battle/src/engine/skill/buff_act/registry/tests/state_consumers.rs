use super::*;

#[test]
fn attr_transactions_follow_the_buff_lifecycle_boundary() {
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffAdded),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffChanged),
        PublicationPhase::AfterPublish
    );
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffRemoved),
        PublicationPhase::AfterPublish
    );
}

#[test]
fn each_change_attr_requires_exact_attribute_arguments() {
    let definition = find(834, "EachChangeAttr").unwrap();
    let supports = definition.supports.unwrap();

    assert!(supports(&[205, 0, 102, 150]));
    assert!(!supports(&[205, 0, 102]));
    assert!(!supports(&[999, 0, 102, 150]));
    assert!(!supports(&[205, -1, 102, 150]));
    assert!(!supports(&[205, 0, 999, 150]));
    assert!(!supports(&[205, 0, 102, -1]));
    assert!(!supports(&[101, 40, 101, 40]));
    assert!(!supports(&[206, 0, 206, 100]));
}

#[test]
fn sentinel_sub_buff_chain_uses_exact_state_consumers() {
    assert!(has_destination(
        932,
        "FixAttrBySubBuffLayer",
        &[31260151, 201, 300, 0]
    ));
    assert!(has_destination(933, "SubBuff", &[31260201]));
    assert!(has_destination(865, "AddPassiveSkills", &[31260181]));

    assert!(!has_destination(
        932,
        "FixAttrBySubBuffLayer",
        &[31260151, 201, 300]
    ));
    assert!(!has_destination(933, "SubBuff", &[-1]));
    assert!(!has_destination(865, "AddPassiveSkills", &[0]));
    assert!(find(932, "FixTempAttrByBuffLayer").is_none());
    assert!(find(933, "AddPassiveSkills").is_none());
    assert!(find(865, "SubBuff").is_none());
}

#[test]
fn passive_state_consumers_reject_unsupported_argument_shapes() {
    for (id, kind, valid, invalid) in [
        (794, "ModifyMaxBurnLayers", vec![20], vec![]),
        (865, "AddPassiveSkills", vec![31200221], vec![0]),
        (
            932,
            "FixAttrBySubBuffLayer",
            vec![31260151, 201, 300, 0],
            vec![31260151, 999, 300, 0],
        ),
        (933, "SubBuff", vec![31260201], vec![0]),
        (951, "CardNotCalSize", vec![31340161], vec![]),
        (1137, "EntityExSkillNotCalSize", vec![], vec![1]),
        (
            1053,
            "AttrByHeatScale",
            vec![205, 25, 600_000, 100_000],
            vec![205, 25, 600_000, 0],
        ),
    ] {
        assert!(has_destination(id, kind, &valid), "{id} {kind}");
        assert!(!has_destination(id, kind, &invalid), "{id} {kind}");
    }
}

#[test]
fn buff_type_duration_extension_is_an_exact_state_consumer() {
    let definition = find(608, "BuffRoundAddByBuffTypeId").unwrap();

    assert_eq!(definition.kind, BuffActKind::BuffRoundAddByBuffTypeId);
    assert_eq!(
        destination(608, "BuffRoundAddByBuffTypeId", &[6003, 1]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(608, "BuffRoundAddByBuffTypeId", &[]));
    assert!(!has_destination(
        608,
        "BuffRoundAddByBuffTypeId",
        &[6003, 0]
    ));
    assert!(find(608, "BuffRoundAdd").is_none());
}

#[test]
fn layered_attribute_bonus_is_an_exact_static_consumer() {
    let args = [201, 300, 31280114, 75, 4, 206, 0];
    let definition = find(1029, "AddAttrByOtherBuffLayer").unwrap();

    assert_eq!(definition.kind, BuffActKind::AddAttrByOtherBuffLayer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1029, "AddAttrByOtherBuffLayer", &args));
    assert!(!has_destination(
        1029,
        "AddAttrByOtherBuffLayer",
        &args[..6]
    ));
    assert!(find(1036, "AddAttrByOtherBuffLayer").is_some());
}

#[test]
fn layered_attribute_penalty_is_an_exact_static_consumer() {
    let args = [204, -200, 31280114, -75, 4, 206, -400];
    let definition = find(1036, "AddAttrByOtherBuffLayer").unwrap();

    assert_eq!(definition.kind, BuffActKind::AddAttrByOtherBuffLayer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1036, "AddAttrByOtherBuffLayer", &args));
    assert!(!has_destination(
        1036,
        "AddAttrByOtherBuffLayer",
        &args[..6]
    ));
    assert!(find(1029, "AddAttrByOtherBuffLayer").is_some());
}

#[test]
fn field_upgrade_modifier_is_an_exact_static_consumer() {
    let definition = find(1032, "FixElectricUpgrade").unwrap();

    assert_eq!(definition.kind, BuffActKind::FixElectricUpgrade);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1032, "FixElectricUpgrade", &[2, 40, 3]));
    assert!(!has_destination(1032, "FixElectricUpgrade", &[2, 40, 2]));
    assert!(find(1032, "AddAttrByOtherBuffLayer").is_none());
}

#[test]
fn healing_taken_modifier_is_an_exact_static_consumer() {
    let definition = find(601, "Injury").unwrap();

    assert_eq!(definition.kind, BuffActKind::Injury);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(601, "Injury", &[500]));
    assert!(has_destination(601, "Injury", &[-250]));
    assert!(!has_destination(601, "Injury", &[0]));
    let wire = definition.wire.expect("injury has wire markers");
    for phase in [
        super::super::super::wire::WirePhase::Add,
        super::super::super::wire::WirePhase::Refresh,
    ] {
        assert_eq!(
            wire.markers(phase),
            &[sonettobuf::effect_type_enum::EffectType::Injury as i32]
        );
    }
    assert!(find(601, "InjuryAbsorb").is_none());
}
