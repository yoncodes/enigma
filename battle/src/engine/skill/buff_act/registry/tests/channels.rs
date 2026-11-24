use super::*;

#[test]
fn counted_channel_keeps_its_exact_state_event_route() {
    let definition = find(838, "CountContinueChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::CountContinueChannel);
    assert_eq!(
        runtime_event(838, "CountContinueChannel", 1041),
        Some(EventKind::BuffStateChanged)
    );
    assert!(has_destination(
        838,
        "CountContinueChannel",
        &[31000505, 1, 210, 7]
    ));
    assert!(!has_destination(
        838,
        "CountContinueChannel",
        &[31000505, 0, 210, 7]
    ));
    let wire = super::super::super::wire::find(838, "CountContinueChannel").unwrap();
    assert_eq!(
        wire.initial_private_state(&[838, 31000505, 1, 210, 7]),
        Some(7)
    );
    assert_eq!(wire.initial_private_state(&[838, 31000505, 1, 210]), None);
    assert!(find(838, "CastChannel").is_none());
}

#[test]
fn contract_channel_keeps_its_exact_grant_and_round_start_routes() {
    let definition = find(836, "ContractCastChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::ContractCastChannel);
    assert_eq!(definition.transaction.events, &[EventKind::BuffAdded]);
    assert_eq!(
        runtime_publication(836, "ContractCastChannel", EventKind::BuffAdded),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        runtime_event(836, "ContractCastChannel", 104),
        Some(EventKind::RoundStart)
    );
    assert!(has_destination(
        836,
        "ContractCastChannel",
        &[1, 150, 31_000_151, 31_000_441]
    ));
    assert!(!has_destination(
        836,
        "ContractCastChannel",
        &[0, 150, 31_000_151, 31_000_441]
    ));
    assert!(!has_destination(
        836,
        "ContractCastChannel",
        &[1, -150, 31_000_151, 31_000_441]
    ));
    assert!(find(836, "CastChannel").is_none());
}

#[test]
fn bound_channel_lock_keeps_its_exact_state_route() {
    let definition = find(837, "NoneCastChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::NoneCastChannel);
    assert_eq!(
        destination(837, "NoneCastChannel", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(837, "NoneCastChannel", &[1]));
    assert!(find(837, "ContractCastChannel").is_none());
}

#[test]
fn shock_wave_channel_keeps_its_exact_round_end_rule() {
    let definition = find(1031, "ConsumeBuffAddBuffContinueChannel").unwrap();

    assert_eq!(
        definition.kind,
        BuffActKind::ConsumeBuffAddBuffContinueChannel
    );
    assert_eq!(
        runtime_event(1031, "ConsumeBuffAddBuffContinueChannel", 302),
        Some(EventKind::RoundEnd)
    );
    assert!(has_destination(
        1031,
        "ConsumeBuffAddBuffContinueChannel",
        &[31280151, 31280113, 0, 50]
    ));
    assert!(!has_destination(
        1031,
        "ConsumeBuffAddBuffContinueChannel",
        &[31280151, 31280113, 20, 20]
    ));
    assert!(find(1031, "ConsumeBuffContinueChannel").is_none());
}

#[test]
fn buff_owned_charge_keeps_its_exact_state_route() {
    crate::test_support::init_config();
    let definition = find(1139, "MeiLeiErCharge").unwrap();

    assert_eq!(definition.kind, BuffActKind::BuffOwnedCharge);
    assert_eq!(definition.completion_gap, None);
    for (buff_id, raw) in [
        (115370004, "1139#100000#150000#30110131"),
        (31460141, "1139#100000#150000#31460181"),
        (31460142, "1139#100000#150000#31460182"),
        (31460143, "1139#100000#150000#31460183"),
    ] {
        let configured = config::configs::get().skill_buff.get(buff_id).unwrap();
        assert_eq!(configured.features, raw);
        let values = crate::engine::entity::skill::split_ids(&configured.features);
        assert_eq!(
            destination(1139, "MeiLeiErCharge", &values[1..]),
            Some(BuffActDestination::StateConsumer)
        );
    }
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[0, 150_000, 31460183]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 99_999, 31460183]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 150_000, 0]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 150_000, 31460183, 1]
    ));
    assert!(find(1139, "HeatScaleUseSkill").is_none());
}
