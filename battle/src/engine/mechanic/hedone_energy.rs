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
mod tests;
