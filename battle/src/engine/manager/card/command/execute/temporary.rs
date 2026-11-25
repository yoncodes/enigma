use super::*;

pub(super) fn add_temporary(
    manager: &mut CardManager,
    add: CardAddTemporary,
) -> Result<CardChanges, CardCommandError> {
    if add.skill_id <= 0 || add.team_type == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let card = match add.kind {
        TemporaryCardKind::GenericSkill
        | TemporaryCardKind::ConfiguredSkill
        | TemporaryCardKind::PrecastSkill => manager.add_temp_card(add.skill_id),
        TemporaryCardKind::ConfiguredSkill3
        | TemporaryCardKind::RoundStartSkill3
        | TemporaryCardKind::HeroSkill => manager.add_temp_card_for(
            add.target_uid,
            add.skill_id,
            add.hero_id,
            add.reserve_id,
            add.team_type,
        ),
    };
    manager.record_opening_temporary(add.kind, &card);
    let operation = Some(CardChange::SpCardAdd {
        target_uid: add.target_uid,
        skill_id: add.skill_id,
        reserve_id: add.reserve_id,
        team_type: add.team_type,
    });
    Ok(CardChanges {
        operation,
        added: Some(card),
        ..CardChanges::new(
            Some(add.origin),
            match add.kind {
                TemporaryCardKind::GenericSkill => CardChangeKind::GenericTemporaryAdded,
                TemporaryCardKind::ConfiguredSkill | TemporaryCardKind::PrecastSkill => {
                    CardChangeKind::TemporaryAdded
                }
                TemporaryCardKind::ConfiguredSkill3 | TemporaryCardKind::RoundStartSkill3 => {
                    CardChangeKind::ConfiguredSkill3Added
                }
                TemporaryCardKind::HeroSkill => CardChangeKind::HeroTemporaryAdded,
            },
        )
    })
}

pub(super) fn add_crystal(
    manager: &mut CardManager,
    add: CardAddCrystal,
) -> Result<CardChanges, CardCommandError> {
    if add.card.uid.unwrap_or_default() == 0
        || add.card.skill_id.unwrap_or_default() <= 0
        || !add.card.temp_card.unwrap_or_default()
    {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.register_skill_group(add.card.uid.unwrap_or_default(), &add.rank_group);
    let card = manager.add_to_hand(add.card);
    Ok(CardChanges {
        added: Some(card),
        ..CardChanges::new(Some(add.origin), CardChangeKind::CrystalAdded)
    })
}

pub(super) fn add_precast(
    manager: &mut CardManager,
    add: CardAddPrecast,
) -> Result<CardChanges, CardCommandError> {
    if add.card.uid.unwrap_or_default() == 0
        || add.card.skill_id.unwrap_or_default() <= 0
        || !add.card.temp_card.unwrap_or_default()
    {
        return Err(CardCommandError::InvalidCommand);
    }
    let index = manager.hand().len();
    let card = manager.add_to_hand(add.card);
    manager.track_temporary(index);
    let operation = Some(CardChange::PrecastPrinted { card: card.clone() });
    Ok(CardChanges {
        operation,
        added: Some(card),
        ..CardChanges::new(Some(add.origin), CardChangeKind::PrecastAdded)
    })
}

pub(super) fn add_selected_precast(
    manager: &mut CardManager,
    add: CardAddPrecast,
) -> Result<CardChanges, CardCommandError> {
    if add.card.uid.unwrap_or_default() == 0
        || add.card.skill_id.unwrap_or_default() <= 0
        || !add.card.temp_card.unwrap_or_default()
    {
        return Err(CardCommandError::InvalidCommand);
    }
    let target_uid = add.card.uid.unwrap_or_default();
    let card = manager.add_pending_temporary(add.card);
    let operation = Some(CardChange::AddHand {
        target_uid,
        card: card.clone(),
    });
    Ok(CardChanges {
        operation,
        added: Some(card),
        ..CardChanges::new(Some(add.origin), CardChangeKind::GeneratedAdded)
    })
}

pub(super) fn change_to_temporary(
    manager: &mut CardManager,
    change: CardChangeToTemporary,
) -> Result<CardChanges, CardCommandError> {
    if change.skill_id <= 0 || change.team_type == 0 || change.index >= manager.hand().len() {
        return Err(CardCommandError::InvalidCommand);
    }
    let card = manager
        .change_to_temp_card_for(
            change.index,
            change.skill_id,
            change.target_uid,
            change.reserve.clone(),
            change.team_type,
        )
        .ok_or(CardCommandError::InvalidCommand)?;
    let operation = Some(CardChange::ChangeToTemp {
        target_uid: change.target_uid,
        reserve_str: change.reserve,
        team_type: change.team_type,
    });
    Ok(CardChanges {
        operation,
        added: Some(card),
        ..CardChanges::new(Some(change.origin), CardChangeKind::TemporaryChanged)
    })
}

pub(super) fn mark_temporary(
    manager: &mut CardManager,
    change: CardMarkTemporary,
) -> Result<CardChanges, CardCommandError> {
    if !manager.mark_hand_temporary(&change.indices) {
        return Err(CardCommandError::InvalidCommand);
    }
    let operation = Some(CardChange::MarkTemporary {
        indices: change.indices,
        team_type: change.team_type,
        config_effect: change.config_effect,
    });
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(change.origin), CardChangeKind::TemporaryChanged)
    })
}

pub(super) fn persist_temporary(
    manager: &mut CardManager,
    change: CardPersistTemporary,
) -> Result<CardChanges, CardCommandError> {
    if !manager.persist_temporary(&change.indices) {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges::new(
        Some(change.origin),
        CardChangeKind::TemporaryPersisted,
    ))
}

pub(super) fn expire_temporary(manager: &mut CardManager, origin: CommandOrigin) -> CardChanges {
    let expired_indices = manager.expire_temporary();
    CardChanges {
        expired_indices,
        ..CardChanges::new(Some(origin), CardChangeKind::TemporaryExpired)
    }
}
