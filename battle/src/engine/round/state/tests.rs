use super::*;

#[test]
fn round_projection_uses_field_specific_temporary_card_rules() {
    let cards = vec![
        CardInfo {
            uid: Some(1),
            skill_id: Some(1),
            energy: Some(0),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(2),
            temp_card: Some(true),
            card_type: Some(0),
            hero_id: Some(3149),
            energy: Some(0),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(3),
            temp_card: Some(true),
            card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
            hero_id: Some(3149),
            energy: Some(0),
            ..Default::default()
        },
        CardInfo {
            uid: Some(0),
            skill_id: Some(4),
            temp_card: Some(true),
            energy: Some(0),
            ..Default::default()
        },
    ];
    let state = RoundState::default();

    let round = next_round_shell(
        &Fight::default(),
        &state,
        RoundCardSnapshotPolicy::All,
        &cards,
        &cards,
        Some((&cards, &cards)),
        &[],
    );
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };

    assert_eq!(skills(&round.before_cards1), vec![1, 3, 4]);
    assert_eq!(skills(&round.before_cards2), vec![1]);
    assert_eq!(round.team_a_cards1, round_field_cards(&cards));
    assert_eq!(round.team_a_cards2, cards);
}

#[test]
fn finished_round_keeps_current_phase_snapshots_without_next_round_cards() {
    let cards = vec![CardInfo {
        skill_id: Some(1),
        ..Default::default()
    }];
    let state = RoundState {
        is_finish: true,
        ..Default::default()
    };

    let round = next_round_shell(
        &Fight::default(),
        &state,
        RoundCardSnapshotPolicy::CurrentOnly,
        &cards,
        &cards,
        Some((&cards, &cards)),
        &[],
    );

    assert!(round.before_cards1.is_empty());
    assert!(round.team_a_cards1.is_empty());
    assert_eq!(round.before_cards2, round_field_cards(&cards));
    assert_eq!(round.team_a_cards2, round_field_cards(&cards));
}

#[test]
fn continuation_round_omits_phase_snapshots() {
    let state = RoundState::default();

    let round = continuation_round_shell(&Fight::default(), &state, &[]);

    assert!(round.before_cards1.is_empty());
    assert!(round.team_a_cards1.is_empty());
    assert!(round.before_cards2.is_empty());
    assert!(round.team_a_cards2.is_empty());
}

#[test]
fn late_terminal_round_keeps_already_prepared_card_snapshots() {
    let cards = vec![CardInfo {
        skill_id: Some(1),
        ..Default::default()
    }];
    let state = RoundState {
        is_finish: true,
        ..Default::default()
    };

    let round = next_round_shell(
        &Fight::default(),
        &state,
        RoundCardSnapshotPolicy::All,
        &cards,
        &cards,
        Some((&cards, &cards)),
        &[],
    );

    assert_eq!(round.before_cards1, round_field_cards(&cards));
    assert_eq!(round.team_a_cards1, round_field_cards(&cards));
    assert_eq!(round.before_cards2, round_field_cards(&cards));
    assert_eq!(round.team_a_cards2, round_field_cards(&cards));
}
