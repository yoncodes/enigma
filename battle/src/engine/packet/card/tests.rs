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
    assert_eq!(add.effect_num, Some(0));
    assert_eq!(add.team_type, Some(1));
    assert_eq!(add.card_info.as_ref().unwrap().uid, Some(10));
    assert!(add.card_info_list.is_empty());
    assert_eq!(push.effect_type, Some(EffectType::Cardspush as i32));
    assert_eq!(push.card_info_list.len(), 1);
    assert_eq!(temp.effect_type, Some(EffectType::Changetotempcard as i32));
    assert_eq!(temp.reserve_str.as_deref(), Some("5"));
    assert_eq!(compose.effect_type, Some(EffectType::Cardscompose as i32));
    assert_eq!(compose.team_type, Some(0));
    assert_eq!(deal.effect_type, Some(EffectType::Dealcard1 as i32));
}

#[test]
fn configured_skill3_packet_matches_the_captured_single_card_shape() {
    let effect = CardPacket::configured_skill3_card(
        CardInfo {
            uid: Some(237352626),
            skill_id: Some(312451036),
            temp_card: Some(true),
            card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
            hero_id: Some(3124),
            ..Default::default()
        },
        1,
    );

    assert_eq!(effect.target_id, Some(237352626));
    assert_eq!(effect.effect_type, Some(EffectType::Addhandcard as i32));
    assert_eq!(effect.effect_num, Some(0));
    assert_eq!(effect.config_effect, Some(0));
    assert_eq!(effect.reserve_id, Some(0));
    assert_eq!(effect.team_type, Some(1));
    assert!(effect.card_info_list.is_empty());
    let card = effect.card_info.unwrap();
    assert_eq!(card.uid, Some(237352626));
    assert_eq!(card.skill_id, Some(312451036));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(
        card.card_type,
        Some(sonettobuf::card_info::CardType::Skill3 as i32)
    );
    assert_eq!(card.hero_id, Some(3124));
    assert_eq!(card.energy, Some(0));
}

#[test]
fn owned_generated_card_projects_as_a_normal_single_card() {
    let effect = CardPacket::from_change(CardChange::OwnedAddHand {
        card: CardInfo {
            uid: Some(237352626),
            skill_id: Some(312451031),
            temp_card: Some(false),
            hero_id: Some(3124),
            ..Default::default()
        },
        team_type: 1,
    });

    assert_eq!(effect.target_id, Some(237352626));
    assert_eq!(effect.effect_type, Some(EffectType::Addhandcard as i32));
    assert_eq!(effect.effect_num, Some(0));
    assert_eq!(effect.team_type, Some(1));
    assert!(effect.card_info_list.is_empty());
    let card = effect.card_info.unwrap();
    assert_eq!(card.hero_id, Some(3124));
    assert_eq!(card.temp_card, Some(false));
    assert_eq!(card.card_type, Some(0));
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
fn card_energy_change_projects_changed_positions_and_deltas() {
    let before = vec![
        CardInfo {
            energy: Some(2),
            ..Default::default()
        },
        CardInfo {
            energy: Some(4),
            ..Default::default()
        },
    ];
    let after = vec![
        CardInfo {
            energy: Some(3),
            ..Default::default()
        },
        CardInfo {
            energy: Some(6),
            ..Default::default()
        },
    ];

    let effect = CardPacket::change_card_energy(&before, &after, 1).unwrap();

    assert_eq!(
        effect.effect_type,
        Some(EffectType::Changecardenergy as i32)
    );
    assert_eq!(effect.effect_num, Some(1));
    assert_eq!(effect.reserve_str.as_deref(), Some("1#1|2#2"));
    assert!(effect.card_info_list.is_empty());
}

#[test]
fn card_packets_render_absent_scalar_metadata_as_wire_defaults() {
    let effect = CardPacket::use_cards(&[crate::engine::round::command::PlayerQueuedAction::Card(
        Box::new(CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        }),
    )]);
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
