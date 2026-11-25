use super::*;

pub(super) fn invalidate_played(
    manager: &mut CardManager,
    invalid: CardInvalidatePlayed,
) -> Result<CardChanges, CardCommandError> {
    let mut operation = None;
    let card = manager
        .invalidate_played(invalid.card_index, invalid.restore)
        .ok_or(CardCommandError::PlayedCardNotFound)?;
    if invalid.restore {
        operation = Some(CardChange::AddHand {
            target_uid: card.uid.unwrap_or_default(),
            card: card.clone(),
        });
    }
    Ok(CardChanges {
        operation,
        added: invalid.restore.then_some(card),
        ..CardChanges::new(Some(invalid.origin), CardChangeKind::PlayedInvalidated)
    })
}

pub(super) fn rank_up_queued(
    manager: &mut CardManager,
    origin: CommandOrigin,
    after_card_index: i32,
    count: i32,
    levels: i32,
) -> Result<CardChanges, CardCommandError> {
    if after_card_index <= 0 || count <= 0 || levels <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.rank_up_played_after(after_card_index, count, levels);
    Ok(CardChanges::new(
        Some(origin),
        CardChangeKind::QueuedRankChanged,
    ))
}

pub(super) fn rewrite_queued_ranks(
    manager: &mut CardManager,
    origin: CommandOrigin,
    upgrades: Vec<QueuedCardRankUp>,
) -> Result<CardChanges, CardCommandError> {
    let mut rank_results = Vec::new();
    if upgrades.is_empty()
        || upgrades
            .iter()
            .any(|upgrade| upgrade.card_index <= 0 || upgrade.levels <= 0)
    {
        return Err(CardCommandError::InvalidCommand);
    }
    rank_results.extend(
        upgrades
            .into_iter()
            .filter_map(|upgrade| manager.rewrite_played_rank(upgrade.card_index, upgrade.levels))
            .map(|change| CardRankResult::Changed(Box::new(change))),
    );
    Ok(CardChanges {
        rank_results,
        ..CardChanges::new(Some(origin), CardChangeKind::QueuedRanksChanged)
    })
}

pub(super) fn change_queued_ranks(
    manager: &mut CardManager,
    origin: CommandOrigin,
    changes: Vec<QueuedCardRankChange>,
) -> Result<CardChanges, CardCommandError> {
    let mut rank_results = Vec::new();
    if changes.is_empty()
        || changes
            .iter()
            .any(|change| change.card_index <= 0 || change.levels == 0)
    {
        return Err(CardCommandError::InvalidCommand);
    }
    for change in changes {
        match manager.change_played_rank(change.card_index, change.levels) {
            Ok(applied) => rank_results.push(CardRankResult::Changed(Box::new(applied))),
            Err(failure) => rank_results.push(CardRankResult::Failed(failure)),
        }
    }
    Ok(CardChanges {
        rank_results,
        ..CardChanges::new(Some(origin), CardChangeKind::QueuedRanksChanged)
    })
}

pub(super) fn complete_action_phase(
    manager: &mut CardManager,
    origin: CommandOrigin,
    carried_cards: &[(i64, i32)],
) -> CardChanges {
    manager.complete_action_phase(carried_cards);
    CardChanges::new(Some(origin), CardChangeKind::ActionPhaseCompleted)
}

pub(super) fn resolve_played_ranks(
    manager: &mut CardManager,
    origin: CommandOrigin,
) -> CardChanges {
    let mut rank_results = Vec::new();
    rank_results.extend(
        manager
            .resolve_played_ranks()
            .into_iter()
            .map(|change| CardRankResult::Changed(Box::new(change))),
    );
    CardChanges {
        rank_results,
        ..CardChanges::new(Some(origin), CardChangeKind::PlayedRanksResolved)
    }
}

pub(super) fn record_cast_channel(
    manager: &mut CardManager,
    record: CardRecordCastChannel,
) -> Result<CardChanges, CardCommandError> {
    if record.buff_uid <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.record_cast_channel(record.buff_uid, record.cards);
    Ok(CardChanges::new(
        Some(record.origin),
        CardChangeKind::CastChannelRecorded,
    ))
}

pub(super) fn resolve_cast_channel(
    manager: &mut CardManager,
    origin: CommandOrigin,
    buff_uid: i64,
) -> Result<CardChanges, CardCommandError> {
    if buff_uid <= 0 || !manager.resolve_cast_channel(buff_uid) {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges::new(
        Some(origin),
        CardChangeKind::CastChannelResolved,
    ))
}

pub(super) fn remove_cast_channel(
    manager: &mut CardManager,
    origin: CommandOrigin,
    buff_uid: i64,
) -> Result<CardChanges, CardCommandError> {
    if buff_uid <= 0 || !manager.remove_cast_channel(buff_uid) {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges::new(
        Some(origin),
        CardChangeKind::CastChannelRemoved,
    ))
}

pub(super) fn queue_use_card(
    manager: &mut CardManager,
    queue: CardQueueUse,
) -> Result<CardChanges, CardCommandError> {
    if queue.card_index <= 0 || queue.card.skill_id.unwrap_or_default() <= 0 || queue.team_type == 0
    {
        return Err(CardCommandError::InvalidCommand);
    }
    let queued = QueuedUseCard {
        card_index: queue.card_index,
        card: queue.card,
        team_type: queue.team_type,
        source_skill_id: queue.source_skill_id,
        reserve_id: queue.reserve_id,
        action: queue.action,
    };
    manager.queue_use_card(queued.clone());
    let queued_use_card = Some(queued);
    Ok(CardChanges {
        queued_use_card,
        ..CardChanges::new(Some(queue.origin), CardChangeKind::UseCardQueued)
    })
}
