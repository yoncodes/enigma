use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1158, "HedoneHurtCounter"),
    }
}

fn record(amount: i32) -> HedoneCounterCommand {
    HedoneCounterCommand::Record(HedoneCounterRecord {
        origin: origin(),
        buff_uid: 10,
        visible: 0,
        amount,
    })
}

fn settle() -> HedoneCounterSettle {
    HedoneCounterSettle {
        origin: origin(),
        buff_uid: 10,
        visible: 0,
        retained_rounds: 4,
    }
}

#[test]
fn retains_only_the_configured_round_window() {
    let mut manager = HedoneCounterManager::default();
    for amount in [10, 20, 30, 40] {
        manager.execute(record(amount)).unwrap();
        manager
            .execute(HedoneCounterCommand::Settle(settle()))
            .unwrap();
    }
    assert_eq!(manager.value(10, 0), Ok(100));

    manager.execute(record(50)).unwrap();
    manager
        .execute(HedoneCounterCommand::Settle(settle()))
        .unwrap();
    assert_eq!(manager.value(10, 0), Ok(140));
}

#[test]
fn rejects_a_visible_counter_without_exact_round_history() {
    let mut manager = HedoneCounterManager::default();
    let command = HedoneCounterCommand::Record(HedoneCounterRecord {
        origin: origin(),
        buff_uid: 10,
        visible: 300,
        amount: 20,
    });
    assert_eq!(
        manager.execute(command),
        Err(HedoneCounterError::MissingHistory {
            buff_uid: 10,
            visible: 300
        })
    );
    assert_eq!(
        manager.settled_value(HedoneCounterSettle {
            visible: 300,
            ..settle()
        }),
        Err(HedoneCounterError::MissingHistory {
            buff_uid: 10,
            visible: 300
        })
    );

    manager
        .execute(HedoneCounterCommand::Clear {
            origin: origin(),
            buff_uid: 10,
        })
        .unwrap();
    assert_eq!(manager.value(10, 0), Ok(0));
}
