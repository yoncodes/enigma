use super::*;

pub(super) fn setup(
    manager: &mut CardManager,
    setup: CardSetup,
) -> Result<CardChanges, CardCommandError> {
    if setup.deck_num < 0
        || setup
            .draw_pile
            .iter()
            .any(|card| card.temp_card.unwrap_or_default())
    {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.reset_with_draw_pile(setup.hand, setup.draw_pile, setup.deck_num);
    Ok(CardChanges::new(None, CardChangeKind::Setup))
}

pub(super) fn preserve_refill_floor(manager: &mut CardManager) -> CardChanges {
    manager.preserve_refill_floor();
    CardChanges::new(None, CardChangeKind::Setup)
}

pub(super) fn preserve_deck_count(manager: &mut CardManager) -> CardChanges {
    manager.preserve_deck_count();
    CardChanges::new(None, CardChangeKind::Setup)
}

pub(super) fn apply_redeal_keep_ranks(
    manager: &mut CardManager,
    redeal: CardRedealKeepRanks,
) -> Result<CardChanges, CardCommandError> {
    if redeal.replacements.len() != manager.redealable_count() {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.redeal_keep_ranks(redeal.replacements);
    Ok(CardChanges {
        team_cards: Some(manager.team_cards().to_vec()),
        ..CardChanges::new(Some(redeal.origin), CardChangeKind::RedealtKeepRanks)
    })
}

pub(super) fn draw(
    manager: &mut CardManager,
    draw: CardDraw,
) -> Result<CardChanges, CardCommandError> {
    let mut operation = None;
    if draw.count == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let cards = manager.draw(draw.count);
    if !cards.is_empty() {
        operation = Some(CardChange::CardsPush {
            cards: manager.hand().to_vec(),
            team_type: 1,
        });
    }
    Ok(CardChanges {
        operation,
        drawn: cards,
        ..CardChanges::new(Some(draw.origin), CardChangeKind::Drawn)
    })
}

pub(super) fn deal_opening(
    manager: &mut CardManager,
    draw: CardOpeningDraw,
) -> Result<CardChanges, CardCommandError> {
    if draw.cards.is_empty() || !manager.deal_opening_cards(&draw.cards, draw.normal_deck_draws) {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges {
        drawn: draw.cards,
        ..CardChanges::new(Some(draw.origin), CardChangeKind::OpeningDrawn)
    })
}

pub(super) fn recycle_draw_pile(
    manager: &mut CardManager,
    origin: CommandOrigin,
    team_type: i32,
) -> Result<CardChanges, CardCommandError> {
    if team_type == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let deck_num = manager
        .recycle_draw_pile()
        .ok_or(CardCommandError::InvalidCommand)?;
    let operation = Some(CardChange::DeckCount {
        deck_num,
        team_type,
    });
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(origin), CardChangeKind::DrawPileRecycled)
    })
}

pub(super) fn refill_one(
    manager: &mut CardManager,
    refill: CardRefillOne,
) -> Result<CardChanges, CardCommandError> {
    if refill.card.skill_id.unwrap_or_default() <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    if refill.consume_draw_pile && !manager.consume_draw_card(&refill.card) {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.add_to_hand(refill.card.clone());
    manager.record_phase_refill(refill.stage, refill.card.clone());
    let owners = manager.compose_adjacent();
    if refill.consume_deck {
        manager.set_deck_num(manager.deck_num().saturating_sub(1));
    }
    Ok(CardChanges {
        drawn: vec![refill.card],
        composed_owners: owners,
        ..CardChanges::new(Some(refill.origin), CardChangeKind::Refilled)
    })
}

pub(super) fn rank_up_deck_range(
    manager: &mut CardManager,
    change: CardDeckRankUpRange,
) -> Result<CardChanges, CardCommandError> {
    let mut rank_results = Vec::new();
    rank_results.extend(
        manager
            .rank_up_deck_range(change.from, change.to, change.rank_delta)?
            .into_iter()
            .map(|change| CardRankResult::Changed(Box::new(change))),
    );
    Ok(CardChanges {
        rank_results,
        ..CardChanges::new(Some(change.origin), CardChangeKind::DeckTopRanksChanged)
    })
}

pub(super) fn begin_post_action_refill(
    manager: &mut CardManager,
    origin: CommandOrigin,
) -> CardChanges {
    manager.begin_post_action_refill();
    CardChanges::new(Some(origin), CardChangeKind::PostActionRefillStarted)
}
