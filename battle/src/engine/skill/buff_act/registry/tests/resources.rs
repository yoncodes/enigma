use super::*;

#[test]
fn moxie_loss_keeps_its_exact_routes() {
    assert_eq!(
        runtime_event(605, "ExPointDel", 302),
        Some(EventKind::RoundEnd)
    );
    assert!(has_destination(605, "ExPointDel", &[1]));
    assert!(!has_destination(605, "ExPointDel", &[0]));

    assert_eq!(
        runtime_event(609, "ExPointDel", 2081),
        Some(EventKind::SkillCast)
    );
    assert!(has_destination(609, "ExPointDel", &[1]));
    assert!(!has_destination(609, "ExPointDel", &[0]));
}

#[test]
fn moxie_reduction_immunity_keeps_its_exact_static_identity() {
    let definition = find(509, "ImmunityExpointChange").unwrap();
    assert_eq!(definition.kind, BuffActKind::MoxieReductionImmunity);
    assert!(definition.state.consumer);
    assert!(has_destination(509, "ImmunityExpointChange", &[]));
    assert!(find(509, "ExPointCantAdd").is_none());
}

#[test]
fn special_moxie_cap_keeps_its_exact_transaction_identity() {
    let definition = find(832, "SpExPointMaxAdd").unwrap();

    assert_eq!(definition.kind, BuffActKind::SpExPointMaxAdd);
    assert_eq!(
        definition.transaction.events,
        &[
            EventKind::BuffAdded,
            EventKind::BuffChanged,
            EventKind::BuffRemoved,
        ]
    );
    assert!(has_destination(832, "SpExPointMaxAdd", &[7]));
    assert!(has_destination(832, "SpExPointMaxAdd", &[7, 3]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[0, 3]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[-7]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[7, -3]));
    assert!(find(832, "ExPointMaxAdd").is_none());
}

#[test]
fn transferred_moxie_keeps_its_exact_state_route() {
    let definition = find(833, "TransferAddExPoint").unwrap();

    assert_eq!(definition.kind, BuffActKind::TransferAddExPoint);
    assert_eq!(
        destination(833, "TransferAddExPoint", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(833, "TransferAddExPoint", &[1]));
    assert!(find(833, "ExPointCantAdd").is_none());
}
