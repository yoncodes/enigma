use sonettobuf::ActEffect;

use crate::engine::{
    fight::versions::RedealWireLayout,
    manager::card::{CARD_PLAY_ORIGIN, CardChangeKind, CardChanges, CardStateSnapshot},
    packet::{
        card::{CardPacket, sovereign},
        effect::EffectPacket,
    },
};

use super::ProjectionError;

pub(super) fn project_card(
    changes: &CardChanges,
    redeal_layout: RedealWireLayout,
) -> Result<Vec<ActEffect>, ProjectionError> {
    Ok(match changes.kind {
        CardChangeKind::Setup
        | CardChangeKind::AiSkillPrepared
        | CardChangeKind::ActionPhaseCompleted
        | CardChangeKind::PostActionRefillStarted => Vec::new(),
        CardChangeKind::AiQueueRefreshed => Vec::new(),
        CardChangeKind::AiQueueSet | CardChangeKind::AiCastRecorded => Vec::new(),
        CardChangeKind::TeamCardsSet => Vec::new(),
        CardChangeKind::OwnerSkillsReplaced => {
            let Some(entity) = changes.entity.as_ref() else {
                return Ok(Vec::new());
            };
            let owner_uid = entity.uid.unwrap_or_default();
            let team_type = entity.team_type.unwrap_or_default();
            let mut effects = changes
                .before
                .iter()
                .zip(&changes.after)
                .enumerate()
                .filter(|(_, (before, after))| {
                    before.uid == Some(owner_uid)
                        && before.uid == after.uid
                        && before.skill_id != after.skill_id
                })
                .map(|(index, (_, after))| {
                    CardPacket::convert_owner_skill_card(owner_uid, index, after.clone(), team_type)
                })
                .collect::<Vec<_>>();
            effects.push(EffectPacket::hero_upgrade(owner_uid, 0, entity.clone()));
            effects
        }
        CardChangeKind::AiOwnerRemoved => changes
            .owner_removal
            .map(|removal| EffectPacket::remove_entity_cards(removal.owner_uid, removal.team_type))
            .into_iter()
            .collect(),
        CardChangeKind::AiCardsInvalidated => changes
            .ai_invalidations
            .iter()
            .map(|invalidated| {
                CardPacket::card_invalid(
                    invalidated.card_index,
                    invalidated.team_type,
                    invalidated.config_effect,
                )
            })
            .collect(),
        CardChangeKind::InitialSovereignBoost => {
            let boost = changes
                .sovereign_boost
                .as_ref()
                .ok_or(ProjectionError::Card(changes.kind))?;
            sovereign::initial_boost(boost)
        }
        CardChangeKind::SovereignShifted => {
            let boost = changes
                .sovereign_boost
                .as_ref()
                .ok_or(ProjectionError::Card(changes.kind))?;
            sovereign::initial_boost(boost)
        }
        CardChangeKind::Moved => Vec::new(),
        CardChangeKind::SovereignUnlocked => {
            let change = changes
                .operation
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![CardPacket::from_change(change)]
        }
        CardChangeKind::HedoneUnlocked => {
            let change = changes
                .operation
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![CardPacket::from_change(change)]
        }
        CardChangeKind::SovereignCharged => {
            let change = changes
                .operation
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![CardPacket::from_change(change)]
        }
        CardChangeKind::Dissolved => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        CardChangeKind::Composed if changes.origin == Some(CARD_PLAY_ORIGIN) => Vec::new(),
        CardChangeKind::Composed => (!changes.composed_owners.is_empty())
            .then(|| CardPacket::cards_compose(Vec::new()))
            .into_iter()
            .collect(),
        CardChangeKind::Played => Vec::new(),
        CardChangeKind::CastChannelRecorded
        | CardChangeKind::CastChannelResolved
        | CardChangeKind::CastChannelRemoved => Vec::new(),
        CardChangeKind::QueuedRankChanged => Vec::new(),
        CardChangeKind::DeckTopRanksChanged => Vec::new(),
        CardChangeKind::HandRankChanged => {
            let entity = changes
                .entity
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            let config_effect = changes
                .origin
                .map(|origin| match origin.domain {
                    crate::engine::skill::rule::RuleDomain::BuffAct => 0,
                    _ => origin.key.opcode,
                })
                .ok_or(ProjectionError::Card(changes.kind))?;
            changes
                .rank_results
                .iter()
                .filter_map(|result| match result {
                    crate::engine::manager::card::CardRankResult::Changed(change) => Some(
                        CardPacket::hand_rank_change(change, entity.clone(), config_effect),
                    ),
                    crate::engine::manager::card::CardRankResult::Failed(_) => None,
                })
                .collect()
        }
        CardChangeKind::QueuedRanksChanged => changes
            .rank_results
            .iter()
            .map(|result| match result {
                crate::engine::manager::card::CardRankResult::Changed(change) => {
                    CardPacket::play_around_rank_change(change)
                }
                crate::engine::manager::card::CardRankResult::Failed(failure) => {
                    CardPacket::play_around_rank_failure(failure)
                }
            })
            .collect(),
        CardChangeKind::PlayedRanksResolved => changes
            .rank_results
            .iter()
            .filter_map(|result| match result {
                crate::engine::manager::card::CardRankResult::Changed(change) => {
                    Some(CardPacket::play_around_rank_change(change))
                }
                crate::engine::manager::card::CardRankResult::Failed(_) => None,
            })
            .collect(),
        CardChangeKind::UseCardQueued => {
            let queued = changes
                .queued_use_card
                .as_ref()
                .expect("queued use-card changes retain their historical card");
            vec![CardPacket::add_use_card(queued)]
        }
        CardChangeKind::EnergyAllocated => {
            vec![CardPacket::allocate_card_energy(changes.after.clone(), 1)]
        }
        CardChangeKind::EnergyChanged => {
            CardPacket::change_card_energy(&changes.before, &changes.after, 1)
                .into_iter()
                .collect()
        }
        CardChangeKind::EnergyCleared => {
            vec![CardPacket::clear_card_energy(changes.after.clone(), 1)]
        }
        CardChangeKind::HandLimitChanged => {
            let (target_uid, limit) = changes
                .hand_limit
                .ok_or(ProjectionError::Card(changes.kind))?;
            let config_effect = changes
                .origin
                .map(|origin| origin.key.opcode)
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![EffectPacket::card_hand_limit(
                target_uid,
                limit,
                config_effect,
            )]
        }
        CardChangeKind::HandLimitCleared => Vec::new(),
        CardChangeKind::ConsumedForEffect => match changes.consumption_kind {
            Some(crate::engine::manager::card::CardConsumptionKind::PaperCircle) => {
                EffectPacket::paper_circle_card_remove(
                    changes
                        .consumed_owner_uid
                        .ok_or(ProjectionError::Card(changes.kind))?,
                    &changes.consumed_indices,
                )
            }
            Some(crate::engine::manager::card::CardConsumptionKind::Hedone) => {
                vec![EffectPacket::hedone_card_remove(&changes.consumed_indices)]
            }
            _ => vec![EffectPacket::card_remove(&changes.consumed_indices)],
        },
        CardChangeKind::GeneratedAdded
        | CardChangeKind::OwnedGeneratedAdded
        | CardChangeKind::SovereignInserted => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        CardChangeKind::HedoneAdded => {
            let operation = changes
                .operation
                .clone()
                .expect("Hedone card commits retain their exact operation");
            let crate::engine::manager::card::CardChange::HedoneAdd {
                card,
                index,
                team_type,
            } = operation.clone()
            else {
                panic!("Hedone card commits use HedoneAdd operations")
            };
            vec![
                CardPacket::from_change(operation),
                CardPacket::update_data(index, card, team_type),
            ]
        }
        CardChangeKind::UniversalAdded => changes
            .after
            .iter()
            .skip(changes.before.len())
            .filter_map(|card| card.skill_id)
            .map(CardPacket::universal_card)
            .collect(),
        CardChangeKind::RedealtKeepRanks => {
            vec![CardPacket::redeal_keep_ranks(
                changes
                    .after
                    .iter()
                    .chain(changes.team_cards.as_deref().unwrap_or_default())
                    .cloned()
                    .collect(),
                changes
                    .origin
                    .map(|origin| origin.key.opcode)
                    .unwrap_or_default(),
                redeal_layout,
            )]
        }
        CardChangeKind::GenericTemporaryAdded => {
            let operation = changes
                .operation
                .clone()
                .expect("generic temporary-card commits retain their exact operation");
            let crate::engine::manager::card::CardChange::SpCardAdd {
                target_uid,
                team_type,
                ..
            } = operation.clone()
            else {
                panic!("generic temporary-card commits use SpCardAdd operations")
            };
            let config_effect = changes
                .origin
                .map(|origin| origin.key.opcode)
                .expect("generic temporary-card commits retain their command origin");
            let mut added = CardPacket::from_change(operation);
            added.config_effect = Some(config_effect);
            let mut temporary = CardPacket::change_to_temp_card(
                target_uid,
                changes.after.len().to_string(),
                team_type,
            );
            temporary.config_effect = Some(config_effect);
            vec![added, temporary]
        }
        CardChangeKind::TemporaryAdded | CardChangeKind::TemporaryChanged => {
            let operation = changes
                .operation
                .clone()
                .expect("temporary-card commits retain their exact operation");
            let mut effects = vec![CardPacket::from_change(operation.clone())];
            if changes.kind == CardChangeKind::TemporaryAdded
                && let crate::engine::manager::card::CardChange::SpCardAdd {
                    target_uid,
                    team_type,
                    ..
                } = operation
            {
                effects.push(CardPacket::change_to_temp_card(
                    target_uid,
                    changes.after.len().to_string(),
                    team_type,
                ));
            }
            effects
        }
        CardChangeKind::HeroTemporaryAdded => {
            let card = changes
                .added
                .clone()
                .expect("hero temporary-card commits retain their added card");
            let operation = changes
                .operation
                .as_ref()
                .expect("hero temporary-card commits retain their exact operation");
            let crate::engine::manager::card::CardChange::SpCardAdd { team_type, .. } = operation
            else {
                panic!("hero temporary-card commits use SpCardAdd operations")
            };
            vec![CardPacket::hero_temp_card(card, *team_type)]
        }
        CardChangeKind::ConfiguredSkill3Added => {
            let card = changes
                .added
                .clone()
                .expect("configured Skill 3 commits retain their added card");
            let operation = changes
                .operation
                .as_ref()
                .expect("configured Skill 3 commits retain their exact operation");
            let crate::engine::manager::card::CardChange::SpCardAdd { team_type, .. } = operation
            else {
                panic!("configured Skill 3 commits use SpCardAdd operations")
            };
            vec![CardPacket::configured_skill3_card(card, *team_type)]
        }
        CardChangeKind::Enchanted => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        CardChangeKind::TemporaryExpired => (!changes.expired_indices.is_empty())
            .then(|| {
                let mut effect = EffectPacket::card_remove(&changes.expired_indices);
                effect.json_param = Some(r#"{"buffActActiveIds":""}"#.to_owned());
                effect
            })
            .into_iter()
            .collect(),
        CardChangeKind::TemporaryPersisted => Vec::new(),
        CardChangeKind::Drawn => {
            vec![CardPacket::cards_push(changes.after.clone(), 1)]
        }
        CardChangeKind::OpeningDrawn => Vec::new(),
        CardChangeKind::DrawPileRecycled => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        CardChangeKind::CrystalAdded => changes
            .added
            .clone()
            .map(EffectPacket::crystal_add_card)
            .into_iter()
            .collect(),
        CardChangeKind::PrecastAdded => {
            let operation = changes
                .operation
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![CardPacket::from_change(operation)]
        }
        CardChangeKind::Refilled => Vec::new(),
        CardChangeKind::UltimateAvailabilityChanged => Vec::new(),
        CardChangeKind::PlayedInvalidated => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
    })
}

pub(super) fn project_snapshot(
    snapshot: &CardStateSnapshot,
    redeal_layout: RedealWireLayout,
) -> Vec<ActEffect> {
    let mut effects = match redeal_layout {
        RedealWireLayout::Version6 => Vec::new(),
        RedealWireLayout::Version7 => {
            vec![CardPacket::redeal_hand_sync(snapshot.hand.clone())]
        }
    };
    effects.push(CardPacket::from_change(
        crate::engine::manager::card::CardChange::DeckCount {
            deck_num: snapshot.deck_num,
            team_type: snapshot.team_type,
        },
    ));
    effects
}
