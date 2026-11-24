use super::*;

pub(super) fn prepare_ai_skill(
    manager: &mut CardManager,
    prepare: CardPrepareAiSkill,
) -> Result<CardChanges, CardCommandError> {
    if prepare.owner_uid == 0 || prepare.skill_id <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.prepared_ai_skills.push(PreparedAiSkill {
        owner_uid: prepare.owner_uid,
        skill_id: prepare.skill_id,
    });
    Ok(CardChanges::new(
        Some(prepare.origin),
        CardChangeKind::AiSkillPrepared,
    ))
}

pub(super) fn refresh_ai_queue(
    manager: &mut CardManager,
    refresh: CardRefreshAiQueue,
) -> CardChanges {
    let owners = manager.refresh_ai_queue(refresh.cards);
    CardChanges {
        composed_owners: owners,
        ..CardChanges::new(Some(refresh.origin), CardChangeKind::AiQueueRefreshed)
    }
}

pub(super) fn set_ai_queue(manager: &mut CardManager, set: CardSetAiQueue) -> CardChanges {
    manager.set_ai_queue(set.cards);
    let ai_queue = Some(manager.ai_queue().to_vec());
    CardChanges {
        ai_queue,
        ..CardChanges::new(Some(set.origin), CardChangeKind::AiQueueSet)
    }
}

pub(super) fn record_ai_cast(manager: &mut CardManager, record: CardRecordAiCast) -> CardChanges {
    manager.ai_casters.push(record.owner_uid);
    CardChanges::new(Some(record.origin), CardChangeKind::AiCastRecorded)
}

pub(super) fn set_team_cards(manager: &mut CardManager, set: CardSetTeamCards) -> CardChanges {
    manager.set_team_cards(set.cards);
    let team_cards = Some(manager.team_cards().to_vec());
    CardChanges {
        team_cards,
        ..CardChanges::new(Some(set.origin), CardChangeKind::TeamCardsSet)
    }
}

pub(super) fn invalidate_ai_owner(
    manager: &mut CardManager,
    invalidate: CardInvalidateAiOwner,
) -> Result<CardChanges, CardCommandError> {
    if invalidate.owner_uid == 0 || invalidate.team_type == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let ai_invalidations = manager
        .invalidate_ai_owner(invalidate.owner_uid)
        .into_iter()
        .map(|(card_index, card)| CardInvalidatedAi {
            card_index,
            team_type: invalidate.team_type,
            config_effect: invalidate.origin.key.opcode,
            card,
        })
        .collect();
    Ok(CardChanges {
        ai_invalidations,
        ..CardChanges::new(Some(invalidate.origin), CardChangeKind::AiCardsInvalidated)
    })
}
