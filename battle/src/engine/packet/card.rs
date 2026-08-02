use sonettobuf::{ActEffect, CardInfo, effect_type_enum::EffectType};

use crate::engine::{fight::versions::RedealWireLayout, manager::card::CardChange};

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
            CardChange::DeckCount {
                deck_num,
                team_type,
            } => Self::card_deck_num(deck_num, team_type),
            CardChange::CardsPush { cards, team_type } => Self::cards_push(cards, team_type),
            CardChange::AddHand { target_uid, card } => Self::add_hand_card(target_uid, card),
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

    pub fn use_cards(cards: Vec<CardInfo>) -> ActEffect {
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
            effect_num: card.skill_id,
            card_info: Some(card.clone()),
            card_info_list: vec![card],
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

    pub fn add_use_card(card_index: i32, card: CardInfo, source_skill_id: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Addusecard as i32),
            effect_num: Some(card_index),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            card_info: Some(wire_card(card)),
            team_type: Some(1),
            effect_num1: Some(source_skill_id),
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
mod tests {
    use super::*;

    #[test]
    fn card_packets_use_card_effect_types_and_payloads() {
        let card = CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        };

        let add = CardPacket::add_hand_card(10, card.clone());
        let push = CardPacket::cards_push(vec![card.clone()], 1);
        let temp = CardPacket::change_to_temp_card(10, "5".to_owned(), 1);
        let compose = CardPacket::from_change(CardChange::CardsCompose {
            cards: vec![card],
            team_type: 1,
        });
        let deal = CardPacket::deal_card1();

        assert_eq!(add.effect_type, Some(EffectType::Addhandcard as i32));
        assert_eq!(add.effect_num, Some(100));
        assert_eq!(add.card_info.as_ref().unwrap().uid, Some(10));
        assert_eq!(push.effect_type, Some(EffectType::Cardspush as i32));
        assert_eq!(push.card_info_list.len(), 1);
        assert_eq!(temp.effect_type, Some(EffectType::Changetotempcard as i32));
        assert_eq!(temp.reserve_str.as_deref(), Some("5"));
        assert_eq!(compose.effect_type, Some(EffectType::Cardscompose as i32));
        assert_eq!(compose.team_type, Some(0));
        assert_eq!(deal.effect_type, Some(EffectType::Dealcard1 as i32));
    }

    #[test]
    fn card_energy_packet_distinguishes_allocation_from_clear() {
        assert_eq!(
            CardPacket::allocate_card_energy(Vec::new(), 1).effect_num1,
            Some(1)
        );
        assert_eq!(
            CardPacket::clear_card_energy(Vec::new(), 1).effect_num1,
            Some(0)
        );
    }

    #[test]
    fn card_packets_render_absent_scalar_metadata_as_wire_defaults() {
        let effect = CardPacket::use_cards(vec![CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        }]);
        let card = &effect.card_info_list[0];

        assert_eq!(card.card_effect, None);
        assert_eq!(card.temp_card, Some(false));
        assert_eq!(card.card_type, Some(0));
        assert_eq!(card.status, Some(0));
        assert_eq!(card.target_uid, Some(0));
        assert_eq!(card.energy, Some(0));
        assert_eq!(card.area_red_or_blue, Some(0));
        assert_eq!(card.heat_id, Some(0));
    }

    #[test]
    fn post_play_hand_snapshot_is_not_a_card_push_operation() {
        let effect = CardPacket::hand_after_use_cards(Vec::new());

        assert_eq!(effect.effect_type, Some(EffectType::Cardspush as i32));
        assert_eq!(effect.effect_num, Some(0));
        assert_eq!(effect.effect_num1, Some(0));
    }

    #[test]
    fn play_around_rank_up_is_owned_by_the_player_team() {
        let effect =
            CardPacket::play_around_rank_change(&crate::engine::manager::card::CardRankChange {
                owner_uid: 10,
                card_index: 2,
                card: CardInfo {
                    card_type: Some(2),
                    ..Default::default()
                },
                rewritten: true,
                rank_delta: 1,
            });

        assert_eq!(effect.team_type, Some(1));
        assert_eq!(effect.effect_num1, Some(1));
        assert_eq!(effect.card_info.unwrap().card_type, Some(0));
    }
}
