use super::*;

pub(super) fn allocate_energy(
    manager: &mut CardManager,
    allocation: CardEnergyAllocation,
) -> Result<CardChanges, CardCommandError> {
    if allocation.limit <= 0
        || allocation.energies.len() != manager.hand().len()
        || allocation
            .energies
            .iter()
            .any(|value| !(0..=allocation.limit).contains(value))
    {
        return Err(CardCommandError::InvalidCommand);
    }
    for (card, energy) in manager.hand_mut().iter_mut().zip(allocation.energies) {
        card.energy = Some(energy);
    }
    Ok(CardChanges::new(
        Some(allocation.origin),
        CardChangeKind::EnergyAllocated,
    ))
}

pub(super) fn change_basic_energy(
    manager: &mut CardManager,
    change: CardEnergyChange,
) -> Result<CardChanges, CardCommandError> {
    if change.delta == 0 || change.count <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.add_basic_card_energy(change.delta, change.count);
    Ok(CardChanges::new(
        Some(change.origin),
        CardChangeKind::EnergyChanged,
    ))
}

pub(super) fn clear_energy(manager: &mut CardManager, origin: CommandOrigin) -> CardChanges {
    manager.clear_energy();
    CardChanges::new(Some(origin), CardChangeKind::EnergyCleared)
}
