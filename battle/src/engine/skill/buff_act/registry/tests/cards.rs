use super::*;

#[test]
fn sovereign_card_generation_has_an_exact_runtime_route() {
    let definition = find(1133, "UnnamedAddSpCard").expect("special-card generation rule");
    assert!(definition.validate_arguments(None, &[31472131, 9, 31470001, 4], None));
    assert_eq!(
        runtime_event(1133, "UnnamedAddSpCard", 105),
        Some(EventKind::RoundStartCard)
    );
    assert!(definition.completion_gap.is_some());
    assert!(definition.runtime.fallible_scoped_handler.is_some());
    for args in [
        &[31472131, 9, 31470001][..],
        &[31472131, 0, 31470001, 4],
        &[31472131, 9, 31470001, 0],
    ] {
        assert!(!definition.validate_arguments(None, args, None));
    }
    assert!(find(1133, "AddSpTempCard").is_none());
}

#[test]
fn card_level_add_keeps_exact_arguments_and_round_start_subscription() {
    let definition = find(701, "CardLevelAdd").expect("CardLevelAdd must be registered exactly");
    assert_eq!(definition.kind, BuffActKind::CardLevelAdd);
    assert_eq!(
        runtime_event(701, "CardLevelAdd", 102),
        Some(EventKind::RoundStart)
    );
    assert!(subscribes_to_event(
        701,
        "CardLevelAdd",
        102,
        EventKind::RoundStart
    ));
    assert!(!subscribes_to_event(
        701,
        "CardLevelAdd",
        102,
        EventKind::RoundEnd
    ));
    assert!(find(701, "CardLevelChange").is_none());
    assert!(find(50011, "CardLevelAdd").is_none());

    let supports = definition
        .supports
        .expect("CardLevelAdd validates arguments");
    for args in [[1], [2], [9]] {
        assert!(supports(&args));
        assert!(has_destination(701, "CardLevelAdd", &args));
    }
    for args in [
        &[][..],
        &[0][..],
        &[3][..],
        &[1, 1][..],
        &[2, 0][..],
        &[9, 1][..],
    ] {
        assert!(!supports(args));
        assert!(!has_destination(701, "CardLevelAdd", args));
    }

    let wire = definition.wire.expect("CardLevelAdd has an add marker");
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Cardleveladd as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn emitter_card_allocation_is_an_exact_state_consumer() {
    let definition = find(879, "EmitterCardAllocateChange").unwrap();
    let args = [1, 300, 1, 2];

    assert_eq!(definition.kind, BuffActKind::EmitterCardAllocateChange);
    assert!(definition.state.consumer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        destination(879, "EmitterCardAllocateChange", &args),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(879, "EmitterCardAllocateChange", &args));
    assert_eq!(runtime_event(879, "EmitterCardAllocateChange", 105), None);
    assert!(!subscribes_to_event(
        879,
        "EmitterCardAllocateChange",
        105,
        EventKind::RoundStartCard
    ));
}

#[test]
fn hedone_counter_keeps_its_exact_hit_and_round_routes() {
    let definition = find(1158, "HedoneHurtCounter").unwrap();

    assert_eq!(definition.kind, BuffActKind::HedoneHurtCounter);
    assert_eq!(definition.state.initial, None);
    assert_eq!(
        definition.setup.routes,
        &[(SetupStage::EnterBattleStatic, 0)]
    );
    assert_eq!(
        definition.setup_frame(SetupStage::EnterBattleStatic, 0),
        (SetupFrameScope::SideSubscriberFrame, 0)
    );
    let wire = definition.wire.unwrap();
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
    assert!(definition.state.consumer);
    assert!(subscribes_to_event(
        1158,
        "HedoneHurtCounter",
        0,
        EventKind::TargetAttacked
    ));
    assert!(subscribes_to_event(
        1158,
        "HedoneHurtCounter",
        0,
        EventKind::RoundEndAfterFinalSettlement
    ));
    assert!(has_destination(
        1158,
        "HedoneHurtCounter",
        &[100, 31560121, 100, 4]
    ));
    assert!(!has_destination(
        1158,
        "HedoneHurtCounter",
        &[100, 31560121, 100]
    ));
    assert!(find(1158, "FoundationCounter").is_none());
}

#[test]
fn hedone_card_marks_only_its_add() {
    let definition = find(1159, "HedoneCard").unwrap();
    let wire = definition.wire.unwrap();

    assert_eq!(definition.kind, BuffActKind::HedoneCard);
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}
