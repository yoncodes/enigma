use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

#[test]
fn legacy_initialization_owns_scalar_progress_and_keeps_indexed_records() {
    let record = FightProgressInfo {
        id: Some(8),
        max: Some(50),
        value: Some(20),
        ..Default::default()
    };
    let mut fight = Fight {
        progress: Some(0),
        progress_max: Some(0),
        progress_list: vec![record],
        ..Default::default()
    };
    let mut manager = FightProgressManager::default();
    manager.seed(&fight);
    let change = manager
        .initialize_legacy(LegacyProgressInit {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60106, "ProgressInit"),
            },
            initial: 10,
            max: 90,
            show_id: 2,
            skill_id: 100,
        })
        .unwrap();

    assert_eq!(
        change.state,
        FightProgressState {
            id: 0,
            value: 10,
            max: 90,
            show_id: 2,
            skill_id: 100,
        }
    );
    manager.sync_fight(&mut fight);
    assert_eq!((fight.progress, fight.progress_max), (Some(10), Some(90)));
    assert_eq!(fight.progress_list, vec![record]);
}

#[test]
fn initialization_owns_the_complete_progress_record() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100015, "ProgressInitNew"),
    };
    let mut manager = FightProgressManager::default();
    let change = manager
        .initialize(FightProgressCommand {
            origin,
            initial: 0,
            max: 100,
            show_id: 10,
            progress_id: 8,
            skill_id: 119381117,
        })
        .unwrap();

    assert_eq!(change.origin, origin);
    assert_eq!(change.state, manager.get(8).unwrap());
    assert_eq!(change.state.value, 0);
    assert_eq!(change.state.max, 100);
    assert!(matches!(
        manager.initialize(FightProgressCommand {
            origin,
            initial: 0,
            max: 100,
            show_id: 10,
            progress_id: 8,
            skill_id: 119381117,
        }),
        Err(FightProgressError::AlreadyInitialized(8))
    ));
}

#[test]
fn snapshot_round_trip_preserves_incomplete_and_duplicate_records() {
    let records = vec![
        FightProgressInfo {
            id: Some(8),
            max: Some(100),
            ..Default::default()
        },
        FightProgressInfo {
            id: Some(8),
            value: Some(50),
            ..Default::default()
        },
        FightProgressInfo::default(),
    ];
    let mut manager = FightProgressManager::default();
    manager.seed(&Fight {
        progress_list: records.clone(),
        ..Default::default()
    });
    let mut fight = Fight::default();

    manager.sync_fight(&mut fight);

    assert_eq!(fight.progress_list, records);
}

#[test]
fn value_change_commits_the_signed_delta() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100016, "ProgressChangeNew"),
    };
    let mut manager = FightProgressManager::default();
    manager
        .initialize(FightProgressCommand {
            origin,
            initial: 100,
            max: 100,
            show_id: 10,
            progress_id: 8,
            skill_id: 119381117,
        })
        .unwrap();

    let change = manager
        .change_value(FightProgressValueCommand {
            origin,
            progress_id: 8,
            delta: -100,
        })
        .unwrap()
        .unwrap();

    assert_eq!(change.before.value, 100);
    assert_eq!(change.after.value, 0);
    assert_eq!(change.delta, -100);
    assert_eq!(manager.get(8).unwrap().value, 0);
}

#[test]
fn value_change_rejects_ambiguous_progress_identity() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100016, "ProgressChangeNew"),
    };
    let mut manager = FightProgressManager::default();
    manager.seed(&Fight {
        progress_list: vec![
            FightProgressState {
                id: 8,
                max: 100,
                value: 10,
                show_id: 10,
                skill_id: 119381117,
            }
            .into(),
            FightProgressState {
                id: 8,
                max: 100,
                value: 20,
                show_id: 10,
                skill_id: 119381117,
            }
            .into(),
        ],
        ..Default::default()
    });

    assert!(matches!(
        manager.change_value(FightProgressValueCommand {
            origin,
            progress_id: 8,
            delta: 5,
        }),
        Err(FightProgressError::AmbiguousProgress(8))
    ));
}

#[test]
fn value_change_rejects_invalid_stored_range() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100016, "ProgressChangeNew"),
    };
    let mut manager = FightProgressManager::default();
    manager.seed(&Fight {
        progress_list: vec![
            FightProgressState {
                id: 8,
                max: -1,
                value: 0,
                show_id: 10,
                skill_id: 119381117,
            }
            .into(),
        ],
        ..Default::default()
    });

    assert!(matches!(
        manager.change_value(FightProgressValueCommand {
            origin,
            progress_id: 8,
            delta: i32::MAX,
        }),
        Err(FightProgressError::InvalidStoredProgress(8))
    ));
}

#[test]
fn value_change_clamps_to_the_configured_range() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100016, "ProgressChangeNew"),
    };
    let mut manager = FightProgressManager::default();
    manager
        .initialize(FightProgressCommand {
            origin,
            initial: 34,
            max: 100,
            show_id: 10,
            progress_id: 8,
            skill_id: 119381117,
        })
        .unwrap();

    let capped = manager
        .change_value(FightProgressValueCommand {
            origin,
            progress_id: 8,
            delta: i32::MAX,
        })
        .unwrap()
        .unwrap();
    assert_eq!((capped.after.value, capped.delta), (100, 66));

    let floored = manager
        .change_value(FightProgressValueCommand {
            origin,
            progress_id: 8,
            delta: i32::MIN,
        })
        .unwrap()
        .unwrap();
    assert_eq!((floored.after.value, floored.delta), (0, -100));
}

#[test]
fn saturated_value_change_has_no_change_result() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100016, "ProgressChangeNew"),
    };
    let mut manager = FightProgressManager::default();
    manager
        .initialize(FightProgressCommand {
            origin,
            initial: 100,
            max: 100,
            show_id: 10,
            progress_id: 8,
            skill_id: 119381117,
        })
        .unwrap();

    assert_eq!(
        manager
            .change_value(FightProgressValueCommand {
                origin,
                progress_id: 8,
                delta: 21,
            })
            .unwrap(),
        None
    );
    assert_eq!(manager.get(8).unwrap().value, 100);
}
