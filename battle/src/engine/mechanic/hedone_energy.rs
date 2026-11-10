#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneEnergyRule {
    pub card_limit: usize,
    pub absorption_start_limit: i32,
    pub unlock_threshold: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneEnergyState {
    pub stored: i32,
    pub maximum: i32,
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HedoneEnergyAbsorption {
    pub card_indices: Vec<usize>,
    pub state: HedoneEnergyState,
}

pub fn absorb(
    rule: HedoneEnergyRule,
    state: HedoneEnergyState,
    mut cards: Vec<(usize, i32)>,
) -> Option<HedoneEnergyAbsorption> {
    if state.stored >= rule.absorption_start_limit {
        return None;
    }
    cards.sort_by_key(|(index, energy)| (*energy, *index));
    let mut indices = Vec::new();
    let mut stored = state.stored;
    for (index, energy) in cards {
        if indices.len() == rule.card_limit {
            break;
        }
        if stored.saturating_add(energy) > state.maximum {
            break;
        }
        indices.push(index);
        stored = stored.saturating_add(energy);
    }
    if indices.is_empty() {
        return None;
    }
    indices.sort_unstable();
    Some(HedoneEnergyAbsorption {
        card_indices: indices,
        state: HedoneEnergyState {
            stored,
            maximum: state.maximum,
            unlocked: stored >= rule.unlock_threshold,
        },
    })
}

#[cfg(test)]
mod tests {
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
}
