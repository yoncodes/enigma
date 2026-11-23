use super::*;

fn rule() -> HedoneEnergyRule {
    HedoneEnergyRule {
        card_limit: 2,
        absorption_start_limit: 4,
        unlock_threshold: 1,
    }
}

#[test]
fn absorption_prefers_lower_energy_then_stops_at_the_card_limit() {
    let result = absorb(
        rule(),
        HedoneEnergyState {
            stored: 0,
            maximum: 7,
            unlocked: false,
        },
        vec![(0, 2), (1, 1), (3, 2), (4, 1)],
    )
    .unwrap();

    assert_eq!(result.card_indices, [1, 4]);
    assert_eq!(result.state.stored, 2);
    assert!(result.state.unlocked);
}

#[test]
fn absorption_does_not_restart_at_the_configured_stop_value() {
    assert!(
        absorb(
            rule(),
            HedoneEnergyState {
                stored: 4,
                maximum: 7,
                unlocked: true,
            },
            vec![(0, 1)],
        )
        .is_none()
    );
}

#[test]
fn absorption_stops_before_exceeding_storage_capacity() {
    let result = absorb(
        HedoneEnergyRule {
            card_limit: 3,
            absorption_start_limit: 8,
            unlock_threshold: 1,
        },
        HedoneEnergyState {
            stored: 6,
            maximum: 12,
            unlocked: true,
        },
        vec![(0, 4), (1, 4), (2, 4)],
    )
    .unwrap();

    assert_eq!(result.card_indices, [0]);
    assert_eq!(result.state.stored, 10);
}
