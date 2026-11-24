use super::*;

pub(super) fn initial_boost(
    manager: &mut CardManager,
    boost: CardInitialSovereignBoost,
) -> Result<CardChanges, CardCommandError> {
    let sovereign_boost = Some(manager.initial_sovereign_boost(boost)?);
    Ok(CardChanges {
        sovereign_boost,
        ..CardChanges::new(Some(boost.origin), CardChangeKind::InitialSovereignBoost)
    })
}

pub(super) fn shift(
    manager: &mut CardManager,
    origin: CommandOrigin,
    owner_uid: i64,
    distance: usize,
    team_type: i32,
) -> Result<CardChanges, CardCommandError> {
    let sovereign_boost = Some(manager.shift_owner_sovereign(owner_uid, distance, team_type)?);
    Ok(CardChanges {
        sovereign_boost,
        ..CardChanges::new(Some(origin), CardChangeKind::SovereignShifted)
    })
}

pub(super) fn unlock(
    manager: &mut CardManager,
    origin: CommandOrigin,
    owner_uid: i64,
    team_type: i32,
) -> Result<CardChanges, CardCommandError> {
    let operation = Some(manager.unlock_sovereign(owner_uid, team_type)?);
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(origin), CardChangeKind::SovereignUnlocked)
    })
}

pub(super) fn charge(
    manager: &mut CardManager,
    origin: CommandOrigin,
    owner_uid: i64,
    kind: SovereignEnhancement,
    team_type: i32,
) -> Result<CardChanges, CardCommandError> {
    let operation = Some(manager.charge_sovereign(owner_uid, kind, team_type)?);
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(origin), CardChangeKind::SovereignCharged)
    })
}

pub(super) fn insert(
    manager: &mut CardManager,
    insert: CardInsertSovereign,
) -> Result<CardChanges, CardCommandError> {
    let operation = manager.insert_sovereign(insert)?;
    let added = operation.as_ref().and_then(|operation| match operation {
        CardChange::InsertHand { card, .. } => Some(card.clone()),
        _ => None,
    });
    Ok(CardChanges {
        operation,
        added,
        ..CardChanges::new(Some(insert.origin), CardChangeKind::SovereignInserted)
    })
}
