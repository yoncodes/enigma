use sonettobuf::{ActEffect, CardInfo, effect_type_enum::EffectType};

use crate::engine::{fight::versions::RedealWireLayout, manager::card::CardChange};

pub(super) mod sovereign;

pub struct CardPacket;

impl CardPacket {
    pub fn enter_fight_deal() -> ActEffect {
        ActEffect {
            effect_type: Some(EffectType::Enterfightdeal as i32),
            ..Default::default()
        }
    }

    pub fn deal_card1() -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Dealcard1 as i32),
            effect_num: Some(0),
            ..Default::default()
        }
    }

    pub fn next_round_cards(cards: Vec<CardInfo>) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardspush as i32),
            effect_num: Some(0),
            card_info_list: wire_cards(cards),
            ..Default::default()
        }
    }

    pub fn from_change(change: CardChange) -> ActEffect {
        match change {
            CardChange::SovereignUnlocked { card, team_type } => {
                Self::update_data(0, card, team_type)
            }
            CardChange::SovereignCharged {
                card,
                index,
                team_type,
            }
            | CardChange::HedoneUnlocked {
                card,
                index,
                team_type,
            } => Self::update_data(index, card, team_type),
            CardChange::InsertHand {
                target_uid,
                card,
                index,
                team_type,
            } => ActEffect {
                target_id: Some(target_uid),
                effect_type: Some(EffectType::Inserthandcard as i32),
                effect_num: Some(i32::try_from(index + 1).expect("committed insertion position")),
                card_info: Some(wire_card(card)),
                team_type: Some(team_type),
                ..Default::default()
            },
            CardChange::DeckCount {
                deck_num,
                team_type,
            } => Self::card_deck_num(deck_num, team_type),
            CardChange::CardsPush { cards, team_type } => Self::cards_push(cards, team_type),
            CardChange::AddHand { target_uid, card } => Self::add_hand_card(target_uid, card),
            CardChange::PrecastPrinted { card } => ActEffect {
                target_id: card.uid,
                effect_type: Some(EffectType::Butterflyaddhandcard as i32),
                effect_num: Some(0),
                card_info: Some(card),
                team_type: Some(1),
                ..Default::default()
            },
            CardChange::OwnedAddHand { card, team_type } => {
                Self::configured_skill3_card(card, team_type)
            }
            CardChange::HedoneAdd {
                card, team_type, ..
            } => Self::hedone_card(card, team_type),
            CardChange::SpCardAdd {
                target_uid,
                skill_id,
                reserve_id,
                team_type,
            } => Self::sp_card_add(target_uid, skill_id, reserve_id, team_type),
            CardChange::ChangeToTemp {
                target_uid,
                reserve_str,
                team_type,
            } => Self::change_to_temp_card(target_uid, reserve_str, team_type),
            CardChange::Enchant {
                cards,
                indices,
                team_type,
            } => Self::enchant_cards(cards, indices, team_type),
            CardChange::MarkTemporary {
                indices,
                team_type,
                config_effect,
            } => Self::mark_temporary(indices, team_type, config_effect),
            CardChange::CardsCompose { cards, .. } => Self::cards_compose(cards),
        }
    }

    pub(crate) fn update_data(index: usize, card: CardInfo, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Updatecarddata as i32),
            effect_num: Some(i32::try_from(index + 1).expect("committed card position")),
            effect_num1: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team_type),
            card_info: Some(wire_card(card)),
            ..Default::default()
        }
    }

    pub fn card_deck_num(deck_num: i32, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Carddecknum as i32),
            effect_num: Some(deck_num),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team_type),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn cards_push(cards: Vec<CardInfo>, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardspush as i32),
            effect_num: Some(team_type),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info_list: wire_cards(cards),
            team_type: Some(0),
            effect_num1: Some(1),
            ..Default::default()
        }
    }

    pub fn use_cards(actions: &[crate::engine::round::command::PlayerQueuedAction]) -> ActEffect {
        let cards = actions
            .iter()
            .map(|action| match action {
                crate::engine::round::command::PlayerQueuedAction::Card(card) => {
                    card.as_ref().clone()
                }
                crate::engine::round::command::PlayerQueuedAction::Conduit => CardInfo {
                    uid: Some(0),
                    skill_id: Some(0),
                    temp_card: Some(false),
                    card_type: Some(sonettobuf::card_info::CardType::Device as i32),
                    hero_id: Some(0),
                    status: Some(0),
                    target_uid: Some(0),
                    energy: Some(0),
                    area_red_or_blue: Some(0),
                    heat_id: Some(0),
                    ..Default::default()
                },
            })
            .collect();
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Usecards as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info_list: wire_cards(cards),
            reserve_str: Some(String::new()),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn hand_after_use_cards(cards: Vec<CardInfo>) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardspush as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info_list: wire_cards(cards),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn add_hand_card(target_uid: i64, card: CardInfo) -> ActEffect {
        let card = wire_card(card);
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Addhandcard as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info: Some(card),
            team_type: Some(1),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn configured_skill3_card(card: CardInfo, team_type: i32) -> ActEffect {
        let card = wire_card(card);
        ActEffect {
            target_id: card.uid,
            effect_type: Some(EffectType::Addhandcard as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info: Some(card),
            team_type: Some(team_type),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn hedone_card(card: CardInfo, team_type: i32) -> ActEffect {
        let mut effect = Self::configured_skill3_card(card, team_type);
        effect.effect_num = Some(1);
        effect
    }

    pub fn convert_owner_skill_card(
        owner_uid: i64,
        hand_index: usize,
        card: CardInfo,
        team_type: i32,
    ) -> ActEffect {
        let card = wire_card(card);
        ActEffect {
            target_id: Some(owner_uid),
            effect_type: Some(EffectType::Cardaconvertcardb as i32),
            effect_num: i32::try_from(hand_index)
                .ok()
                .and_then(|index| index.checked_add(1)),
            config_effect: card.skill_id,
            buff_act_id: Some(0),
            reserve_id: Some(1),
            card_info: Some(card),
            team_type: Some(team_type),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn universal_card(skill_id: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Universalcard as i32),
            effect_num: Some(skill_id),
            team_type: Some(1),
            ..Default::default()
        }
    }

    pub(crate) fn redeal_keep_ranks(
        cards: Vec<CardInfo>,
        config_effect: i32,
        layout: RedealWireLayout,
    ) -> ActEffect {
        match layout {
            RedealWireLayout::Version6 => ActEffect {
                target_id: Some(0),
                effect_type: Some(EffectType::Redealcard as i32),
                config_effect: Some(config_effect),
                team_type: Some(0),
                ..Default::default()
            },
            RedealWireLayout::Version7 => ActEffect {
                target_id: Some(0),
                effect_type: Some(EffectType::Afterredealcard as i32),
                effect_num: Some(0),
                config_effect: Some(0),
                buff_act_id: Some(0),
                reserve_id: Some(0),
                card_info_list: wire_cards(cards),
                team_type: Some(1),
                effect_num1: Some(0),
                ..Default::default()
            },
        }
    }

    pub(crate) fn redeal_hand_sync(cards: Vec<CardInfo>) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardspush as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info_list: wire_cards(cards),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn sp_card_add(
        target_uid: i64,
        skill_id: i32,
        reserve_id: i64,
        team_type: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Spcardadd as i32),
            effect_num: Some(skill_id),
            reserve_id: Some(reserve_id),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn hero_temp_card(card: CardInfo, team_type: i32) -> ActEffect {
        let card = wire_card(card);
        ActEffect {
            target_id: card.uid,
            effect_type: Some(EffectType::Spcardadd as i32),
            card_info: Some(card),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn change_to_temp_card(target_uid: i64, reserve_str: String, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Changetotempcard as i32),
            reserve_str: Some(reserve_str),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn enchant_cards(cards: Vec<CardInfo>, indices: Vec<usize>, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardeffectchange as i32),
            reserve_str: Some(card_indices(indices)),
            card_info_list: wire_cards(cards),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn mark_temporary(indices: Vec<usize>, team_type: i32, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Changetotempcard as i32),
            config_effect: Some(config_effect),
            reserve_str: Some(card_indices(indices)),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn cards_compose(cards: Vec<CardInfo>) -> ActEffect {
        ActEffect {
            effect_type: Some(EffectType::Cardscompose as i32),
            card_info_list: wire_cards(cards),
            team_type: Some(0),
            ..Default::default()
        }
    }

    pub fn card_invalid(card_index: i32, team_type: i32, config_effect: i32) -> ActEffect {
        ActEffect {
            effect_type: Some(EffectType::Cardinvalid as i32),
            effect_num: Some(card_index),
            config_effect: Some(config_effect),
            team_type: Some(team_type),
            ..Default::default()
        }
    }

    pub fn allocate_card_energy(cards: Vec<CardInfo>, team_type: i32) -> ActEffect {
        Self::card_energy(cards, team_type, 1)
    }

    pub fn clear_card_energy(cards: Vec<CardInfo>, team_type: i32) -> ActEffect {
        Self::card_energy(cards, team_type, 0)
    }

    pub fn change_card_energy(
        before: &[CardInfo],
        after: &[CardInfo],
        team_type: i32,
    ) -> Option<ActEffect> {
        let changes = before
            .iter()
            .zip(after)
            .enumerate()
            .filter_map(|(index, (before, after))| {
                let delta = after.energy.unwrap_or_default() - before.energy.unwrap_or_default();
                (delta != 0).then(|| format!("{}#{delta}", index + 1))
            })
            .collect::<Vec<_>>();
        (!changes.is_empty()).then(|| ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Changecardenergy as i32),
            effect_num: Some(team_type),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(changes.join("|")),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        })
    }

    fn card_energy(cards: Vec<CardInfo>, team_type: i32, operation: i32) -> ActEffect {
        ActEffect {
            effect_type: Some(EffectType::Allocatecardenergy as i32),
            effect_num: Some(team_type),
            effect_num1: Some(operation),
            card_info_list: wire_cards(cards),
            team_type: Some(0),
            ..Default::default()
        }
    }

    pub fn add_use_card(queued: &crate::engine::manager::card::QueuedUseCard) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Addusecard as i32),
            effect_num: Some(queued.card_index),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(i64::from(queued.reserve_id)),
            card_info: Some(wire_card(queued.card.clone())),
            team_type: Some(1),
            effect_num1: Some(queued.source_skill_id),
            ..Default::default()
        }
    }

    pub fn play_around_rank_change(
        change: &crate::engine::manager::card::CardRankChange,
    ) -> ActEffect {
        let mut card = change.card.clone();
        card.card_type = Some(0);
        ActEffect {
            target_id: Some(change.owner_uid),
            effect_type: Some(if change.rank_delta < 0 {
                EffectType::Playarounddownrank as i32
            } else {
                EffectType::Playarounduprank as i32
            }),
            effect_num: Some(change.card_index),
            effect_num1: Some(i32::from(change.rewritten)),
            card_info: Some(wire_card(card)),
            team_type: Some(1),
            ..Default::default()
        }
    }

    pub fn hand_rank_change(
        change: &crate::engine::manager::card::CardRankChange,
        entity: sonettobuf::FightEntityInfo,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(i64::from(change.card_index)),
            effect_type: Some(EffectType::Cardlevelchange as i32),
            effect_num: change.card.skill_id,
            entity: Some(entity),
            config_effect: Some(config_effect),
            team_type: Some(1),
            ..Default::default()
        }
    }

    pub fn play_around_rank_failure(
        failure: &crate::engine::manager::card::CardRankFailure,
    ) -> ActEffect {
        let mut card = failure.card.as_ref().clone();
        card.card_type = Some(0);
        ActEffect {
            target_id: Some(failure.owner_uid),
            effect_type: Some(EffectType::Playchangerankfail as i32),
            effect_num: Some(failure.card_index),
            effect_num1: Some(0),
            reserve_str: Some("0".to_owned()),
            card_info: Some(wire_card(card)),
            team_type: Some(1),
            ..Default::default()
        }
    }
}

fn card_indices(indices: Vec<usize>) -> String {
    indices
        .into_iter()
        .filter_map(|index| index.checked_add(1))
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join("#")
}

fn wire_cards(cards: Vec<CardInfo>) -> Vec<CardInfo> {
    cards.into_iter().map(wire_card).collect()
}

fn wire_card(mut card: CardInfo) -> CardInfo {
    card.temp_card.get_or_insert(false);
    card.card_type.get_or_insert(0);
    card.status.get_or_insert(0);
    card.target_uid.get_or_insert(0);
    card.energy.get_or_insert(0);
    card.area_red_or_blue.get_or_insert(0);
    card.heat_id.get_or_insert(0);
    card
}

#[cfg(test)]
mod tests;
