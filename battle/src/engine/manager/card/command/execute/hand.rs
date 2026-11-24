use super::*;

pub(super) fn consume_for_effect(
    manager: &mut CardManager,
    consume: CardConsumeForEffect,
) -> Result<CardChanges, CardCommandError> {
    let consumed = match consume.kind {
        CardConsumptionKind::Hedone => manager.consume_conduit_energy(&consume.indices),
        CardConsumptionKind::Generic | CardConsumptionKind::PaperCircle => {
            manager.consume_for_effect(consume.owner_uid, &consume.indices)
        }
    };
    if !consumed {
        return Err(CardCommandError::InvalidCommand);
    }
    let consumed_indices = consume.indices;
    let consumed_owner_uid = Some(consume.owner_uid);
    let consumption_kind = Some(consume.kind);
    Ok(CardChanges {
        consumed_indices,
        consumed_owner_uid,
        consumption_kind,
        ..CardChanges::new(Some(consume.origin), CardChangeKind::ConsumedForEffect)
    })
}

pub(super) fn move_card(
    manager: &mut CardManager,
    origin: CommandOrigin,
    from_index: usize,
    to_index: usize,
) -> Result<CardChanges, CardCommandError> {
    if !manager.move_card(from_index, to_index) {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges::new(Some(origin), CardChangeKind::Moved))
}

pub(super) fn use_universal(
    manager: &mut CardManager,
    use_card: CardUseUniversal,
) -> Result<CardChanges, CardCommandError> {
    let owner_uid = manager
        .use_universal(use_card.universal_index, use_card.target_index)
        .ok_or(CardCommandError::InvalidCommand)?;
    Ok(CardChanges {
        composed_owners: vec![owner_uid],
        ..CardChanges::new(Some(use_card.origin), CardChangeKind::Composed)
    })
}

pub(super) fn dissolve(
    manager: &mut CardManager,
    origin: CommandOrigin,
    card_index: usize,
) -> Result<CardChanges, CardCommandError> {
    manager
        .dissolve(card_index, None)
        .ok_or(CardCommandError::InvalidCommand)?;
    let operation = Some(CardChange::CardsPush {
        cards: manager.hand().to_vec(),
        team_type: 1,
    });
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(origin), CardChangeKind::Dissolved)
    })
}

pub(super) fn unlock_hedone(
    manager: &mut CardManager,
    unlock: CardUnlockHedone,
) -> Result<CardChanges, CardCommandError> {
    let operation = Some(manager.unlock_hedone(unlock)?);
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(unlock.origin), CardChangeKind::HedoneUnlocked)
    })
}

pub(super) fn add_generated(
    manager: &mut CardManager,
    add: CardAddGenerated,
) -> Result<CardChanges, CardCommandError> {
    let owned = match (add.hero_id, add.team_type) {
        (None, 0) => false,
        (Some(hero_id), team_type) if hero_id > 0 && team_type != 0 => true,
        _ => return Err(CardCommandError::InvalidCommand),
    };
    if add.target_uid == 0 || add.skill_id <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let card = manager.add_to_hand_for(
        add.target_uid,
        CardInfo {
            uid: Some(add.target_uid),
            skill_id: Some(add.skill_id),
            hero_id: add.hero_id,
            temp_card: Some(false),
            ..Default::default()
        },
    );
    let operation = Some(if owned {
        CardChange::OwnedAddHand {
            card: card.clone(),
            team_type: add.team_type,
        }
    } else {
        CardChange::AddHand {
            target_uid: add.target_uid,
            card: card.clone(),
        }
    });
    Ok(CardChanges {
        operation,
        added: Some(card),
        ..CardChanges::new(
            Some(add.origin),
            if owned {
                CardChangeKind::OwnedGeneratedAdded
            } else {
                CardChangeKind::GeneratedAdded
            },
        )
    })
}

pub(super) fn add_hedone(
    manager: &mut CardManager,
    add: CardAddHedone,
) -> Result<CardChanges, CardCommandError> {
    let operation = Some(manager.add_hedone(add)?);
    let card = operation.as_ref().and_then(|operation| match operation {
        CardChange::HedoneAdd { card, .. } => Some(card.clone()),
        _ => None,
    });
    Ok(CardChanges {
        operation,
        added: card,
        ..CardChanges::new(Some(add.origin), CardChangeKind::HedoneAdded)
    })
}

pub(super) fn add_universal(
    manager: &mut CardManager,
    add: CardAddUniversal,
) -> Result<CardChanges, CardCommandError> {
    let Some(skill) = crate::engine::manager::card::UniversalCardSkill::from_rank(add.rank) else {
        return Err(CardCommandError::InvalidCommand);
    };
    if add.count <= 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.add_universal_cards(add.count, skill);
    Ok(CardChanges::new(
        Some(add.origin),
        CardChangeKind::UniversalAdded,
    ))
}

pub(super) fn enchant_hand(
    manager: &mut CardManager,
    change: CardEnchantHand,
) -> Result<CardChanges, CardCommandError> {
    let cards = manager
        .enchant_hand(&change.indices, change.enchant, change.duration)
        .ok_or(CardCommandError::InvalidCommand)?;
    let operation = Some(CardChange::Enchant {
        cards,
        indices: change.indices,
        team_type: change.team_type,
    });
    Ok(CardChanges {
        operation,
        ..CardChanges::new(Some(change.origin), CardChangeKind::Enchanted)
    })
}

pub(super) fn play(
    manager: &mut CardManager,
    play: CardPlay,
) -> Result<CardChanges, CardCommandError> {
    if play.choice.is_none() && manager.visible_card(play.hand_index).is_none() {
        return Err(CardCommandError::InvalidPlayIndex);
    }
    if play
        .planned_skill(manager.visible_card(play.hand_index))
        .is_none()
    {
        return Err(CardCommandError::InvalidPlaySkill);
    }
    manager.check_play_permission(play.hand_index, play.choice.as_ref())?;
    let played = manager
        .play_card_with_record(
            play.hand_index,
            play.target_uid,
            play.chosen_skill_id,
            play.choice,
            play.recorded_skill,
        )
        .ok_or(CardCommandError::PlayedCardNotFound)?;
    Ok(CardChanges {
        played: Some(played),
        ..CardChanges::new(Some(play.origin), CardChangeKind::Played)
    })
}

pub(super) fn set_ultimate_availability(
    manager: &mut CardManager,
    set: CardSetUltimateAvailability,
) -> Result<CardChanges, CardCommandError> {
    if set.card.uid.unwrap_or_default() == 0
        || set.card.skill_id.unwrap_or_default() <= 0
        || !manager.set_ultimate_availability(set.card.clone(), set.available)
    {
        return Err(CardCommandError::InvalidCommand);
    }
    Ok(CardChanges {
        added: set.available.then_some(set.card),
        ..CardChanges::new(
            Some(set.origin),
            CardChangeKind::UltimateAvailabilityChanged,
        )
    })
}

pub(super) fn remove_owner(
    manager: &mut CardManager,
    remove: CardRemoveOwner,
) -> Result<CardChanges, CardCommandError> {
    let mut owner_removal = None;
    if remove.owner_uid == 0 || remove.team_type == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    let owners = manager.remove_owner_cards(remove.owner_uid, remove.team_type);
    if owners.is_some() {
        owner_removal = Some(CardOwnerRemoval {
            owner_uid: remove.owner_uid,
            team_type: remove.team_type,
        });
    }
    Ok(CardChanges {
        owner_removal,
        composed_owners: owners.unwrap_or_default(),
        ..CardChanges::new(Some(remove.origin), CardChangeKind::AiOwnerRemoved)
    })
}

pub(super) fn replace_owner_skills(
    manager: &mut CardManager,
    replace: CardReplaceOwnerSkills,
) -> Result<CardChanges, CardCommandError> {
    if replace.owner_uid == 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.replace_owner_skills(
        replace.owner_uid,
        &replace.base_group1,
        &replace.base_group2,
        &replace.replacement_group1,
        &replace.replacement_group2,
    );
    Ok(CardChanges::new(
        Some(replace.origin),
        CardChangeKind::OwnerSkillsReplaced,
    ))
}

pub(super) fn compose_adjacent(manager: &mut CardManager, origin: CommandOrigin) -> CardChanges {
    let owners = manager.compose_adjacent();
    CardChanges {
        composed_owners: owners,
        ..CardChanges::new(Some(origin), CardChangeKind::Composed)
    }
}

pub(super) fn change_hand_limit(
    manager: &mut CardManager,
    change: CardHandLimitChange,
) -> Result<CardChanges, CardCommandError> {
    if change.delta == 0 || change.resulting_limit < 0 {
        return Err(CardCommandError::InvalidCommand);
    }
    manager.add_hand_limit_bonus(change.delta);
    let hand_limit = Some((change.target_uid, change.resulting_limit));
    Ok(CardChanges {
        hand_limit,
        ..CardChanges::new(Some(change.origin), CardChangeKind::HandLimitChanged)
    })
}

pub(super) fn clear_hand_limit(manager: &mut CardManager, origin: CommandOrigin) -> CardChanges {
    manager.clear_hand_limit_bonus();
    CardChanges::new(Some(origin), CardChangeKind::HandLimitCleared)
}

pub(super) fn rank_up_hand(
    manager: &mut CardManager,
    change: HandCardRankUp,
) -> Result<CardChanges, CardCommandError> {
    let rank_results = vec![CardRankResult::Changed(Box::new(
        manager.rank_up_hand(change.owner_uid, change.hand_index)?,
    ))];
    Ok(CardChanges {
        rank_results,
        ..CardChanges::new(Some(change.origin), CardChangeKind::HandRankChanged)
    })
}
