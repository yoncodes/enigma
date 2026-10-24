use super::*;

#[test]
fn shared_pool_marker_resolves_to_one_exact_gauge_kind() {
    assert_eq!(
        GaugeKind::from_shared_pool_config_effect(0),
        Some(GaugeKind::Bloodtithe)
    );
    assert_eq!(
        GaugeKind::from_shared_pool_config_effect(1),
        Some(GaugeKind::LingeringGlow)
    );
    assert_eq!(GaugeKind::from_shared_pool_config_effect(2), None);
}
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const KEY: GaugeKey = GaugeKey {
    kind: GaugeKind::Bloodtithe,
    owner: GaugeOwner::Team(1),
};
const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::BuffAct,
    key: DefinitionKey::new(953, "BloodPoolTag"),
};
const BATCH: GaugeBatchId = GaugeBatchId::new(ORIGIN, KEY, 0, 1);
const OTHER_BATCH: GaugeBatchId = GaugeBatchId::new(ORIGIN, KEY, 0, 2);
const MISMATCHED_BATCH: GaugeBatchId = GaugeBatchId::new(ORIGIN, KEY, 1, 1);

#[test]
fn enable_change_clamp_and_disable_share_one_owner() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(10) },
        ))
        .unwrap();
    assert!(
        manager
            .execute_command(GaugeCommand::new(
                ORIGIN,
                KEY,
                GaugeOperation::Enable { max: Some(99) },
            ))
            .unwrap()
            .events()
            .is_empty()
    );
    assert_eq!(manager.get(KEY).unwrap().max, Some(10));
    let change = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 12 },
        ))
        .unwrap();

    assert_eq!((change.after, change.overflow), (10, 2));
    assert_eq!(change.events().len(), 1);
    manager
        .execute_command(GaugeCommand::new(ORIGIN, KEY, GaugeOperation::Disable))
        .unwrap();
    assert_eq!(manager.get(KEY), None);
}

#[test]
fn accumulated_input_keeps_its_remainder_in_the_gauge_manager() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();

    let first = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateValue {
                amount: 9_070,
                threshold: 2_968,
            },
        ))
        .unwrap();
    let second = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateValue {
                amount: 2_802,
                threshold: 2_968,
            },
        ))
        .unwrap();

    assert_eq!((first.applied_delta, second.applied_delta), (3, 1));
    assert_eq!(manager.get(KEY).unwrap().current, 4);
}

#[test]
fn accumulated_input_remainder_survives_the_round_boundary() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateValue {
                amount: 2_000,
                threshold: 3_000,
            },
        ))
        .unwrap();

    manager.begin_combat_round();

    let change = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateValue {
                amount: 1_000,
                threshold: 3_000,
            },
        ))
        .unwrap();
    assert_eq!(change.applied_delta, 1);
    assert_eq!(manager.get(KEY).unwrap().current, 1);
}

#[test]
fn opening_setup_uses_the_enabled_limit_for_accumulation_thresholds() {
    let mut manager = GaugeManager::default();
    manager.begin_opening_setup();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeMax { delta: 28 },
        ))
        .unwrap();

    assert_eq!(manager.accumulation_max(KEY), Some(56));
    manager.finish_opening_setup();
    assert_eq!(manager.accumulation_max(KEY), Some(84));
}

#[test]
fn accumulated_progress_is_independent_from_spendable_gauge_value() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    manager
        .execute_command(
            GaugeCommand::new(ORIGIN, KEY, GaugeOperation::ChangeValue { delta: 34 })
                .with_raw_delta(34_480),
        )
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: -17 },
        ))
        .unwrap();

    assert_eq!(manager.get(KEY).unwrap().current, 17);
    assert_eq!(manager.accumulated_value(KEY, 30, 1050), Some(34));
    assert_eq!(
        manager.preview_accumulated_change(KEY, 30, 1050, 5_000),
        Some(39)
    );

    let consumed = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ConsumeAccumulated {
                listener_uid: 30,
                listener_opcode: 1050,
                amount: 10,
            },
        ))
        .unwrap();
    assert!(matches!(
        consumed.events().as_slice(),
        [BattleEvent::GaugeChanged(event)]
            if event.kind == GaugeChangeKind::Accumulated
                && event.applied_delta == -10
                && event.after == 24
    ));
    assert_eq!(manager.get(KEY).unwrap().current, 17);
    assert_eq!(manager.accumulated_value(KEY, 30, 1050), Some(24));
}

#[test]
fn direct_progress_does_not_mutate_the_spendable_gauge() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    let change = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateProgress { raw_amount: 75_000 },
        ))
        .unwrap();

    assert!(matches!(
        change.events().as_slice(),
        [BattleEvent::GaugeChanged(event)]
            if event.kind == GaugeChangeKind::Accumulated
                && event.applied_delta == 75
                && event.after == 75
    ));
    assert_eq!(manager.get(KEY).unwrap().current, 0);
    assert_eq!(manager.accumulated_value(KEY, 30, 1050), Some(75));
}

#[test]
fn raw_value_accumulation_truncates_visible_value_and_keeps_precise_progress() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();

    let changes = (0..3)
        .map(|_| {
            manager
                .execute_command(GaugeCommand::new(
                    ORIGIN,
                    KEY,
                    GaugeOperation::AccumulateRawValue {
                        amount: 4_998,
                        stream: 726,
                    },
                ))
                .unwrap()
                .applied_delta
        })
        .collect::<Vec<_>>();

    assert_eq!(changes, vec![4, 5, 5]);
    assert_eq!(manager.raw_value(KEY), Some(14_994));
    assert_eq!(manager.get(KEY).unwrap().current, 14);
    assert_eq!(manager.accumulated_value(KEY, 30, 1050), Some(14));
}

#[test]
fn fractional_raw_gains_round_against_the_shared_running_total() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 34_063,
                stream: 726,
            },
        ))
        .unwrap();

    let changes = (0..2)
        .map(|_| {
            manager
                .execute_command(GaugeCommand::new(
                    ORIGIN,
                    KEY,
                    GaugeOperation::AccumulateRawValue {
                        amount: 1_666,
                        stream: 771,
                    },
                ))
                .unwrap()
                .applied_delta
        })
        .collect::<Vec<_>>();

    assert_eq!(changes, vec![1, 2]);
    assert_eq!(manager.raw_value(KEY), Some(37_395));
    assert_eq!(manager.get(KEY).unwrap().current, 37);
}

#[test]
fn raw_contribution_plan_assigns_fractional_remainder_in_input_order() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 34_063,
                stream: 726,
            },
        ))
        .unwrap();

    assert_eq!(
        manager.plan_raw_contributions(KEY, &[1_666, 1_666]),
        Some(vec![2, 1])
    );
}

#[test]
fn raw_batch_owns_one_change_and_each_contribution_event() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 34_063,
                stream: 726,
            },
        ))
        .unwrap();

    let commands = vec![
        GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 1_666,
                stream: 771,
            },
        ),
        GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 1_666,
                stream: 771,
            },
        ),
    ];
    assert_eq!(
        manager.start_raw_batch(MISMATCHED_BATCH, commands.clone()),
        Err(GaugeCommandError::InvalidCommand)
    );
    let first = manager.start_raw_batch(BATCH, commands).unwrap();
    assert_eq!(
        manager.continue_raw_batch(OTHER_BATCH),
        Err(GaugeCommandError::InvalidCommand)
    );
    assert_eq!(
        manager.commit_raw_batch(OTHER_BATCH),
        Err(GaugeCommandError::InvalidCommand)
    );
    assert_eq!(
        manager.execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 5 },
        )),
        Err(GaugeCommandError::InvalidCommand)
    );
    let second = manager.continue_raw_batch(BATCH).unwrap();
    let changes = manager.commit_raw_batch(BATCH).unwrap();

    assert_eq!([first.applied_delta, second.applied_delta], [2, 1]);
    assert_eq!(changes.combined.before, 34);
    assert_eq!(changes.combined.applied_delta, 3);
    assert_eq!(changes.combined.after, 37);
    assert_eq!(first.events().len() + second.events().len(), 2);
    assert_eq!(manager.raw_value(KEY), Some(37_395));
}

#[test]
fn raw_value_accumulation_shares_the_gauge_fraction_between_opcodes() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 4_998,
                stream: 726,
            },
        ))
        .unwrap();
    let halo = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::AccumulateRawValue {
                amount: 6_664,
                stream: 771,
            },
        ))
        .unwrap();

    assert_eq!(halo.applied_delta, 7);
    assert_eq!(manager.accumulated_value(KEY, 30, 1050), Some(11));
}

#[test]
fn base_sync_preserves_explicit_max_bonuses() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeMax { delta: 16 },
        ))
        .unwrap();
    let synced = manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::SyncBaseMax { max: 68 },
        ))
        .unwrap();

    assert_eq!(synced.applied_delta, 12);
    assert_eq!(manager.base_max(KEY), Some(68));
    assert_eq!(manager.get(KEY).unwrap().max, Some(84));
}

#[test]
fn precise_value_is_owned_with_the_gauge_state() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(100) },
        ))
        .unwrap();
    manager
        .execute_command(
            GaugeCommand::new(ORIGIN, KEY, GaugeOperation::ChangeValue { delta: 4 })
                .with_raw_delta(4_480),
        )
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: -2 },
        ))
        .unwrap();

    assert_eq!(manager.get(KEY).unwrap().current, 2);
    assert_eq!(manager.raw_value(KEY), Some(2_480));
}

#[test]
fn positive_threshold_listener_can_settle_each_new_crossing_in_one_round() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    manager.begin_combat_round();

    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 8 },
        ))
        .unwrap();
    assert_eq!(manager.settle_positive_threshold(KEY, 20, 1021, 8, 1), 1);
    assert_eq!(manager.settle_positive_threshold(KEY, 20, 1021, 8, 1), 0);

    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 8 },
        ))
        .unwrap();
    assert_eq!(manager.settle_positive_threshold(KEY, 20, 1021, 8, 1), 1);
    assert_eq!(manager.settle_positive_threshold(KEY, 20, 1021, 8, 1), 0);
}

#[test]
fn positive_threshold_listener_does_not_carry_partial_round_progress() {
    let mut manager = GaugeManager::default();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 7 },
        ))
        .unwrap();
    manager.begin_combat_round();

    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 1 },
        ))
        .unwrap();
    assert_eq!(manager.preview_positive_threshold(KEY, 20, 1021, 8, 1), 0);

    manager
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 7 },
        ))
        .unwrap();
    assert_eq!(manager.settle_positive_threshold(KEY, 20, 1021, 8, 1), 1);
}
