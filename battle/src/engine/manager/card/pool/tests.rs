use super::*;
use sonettobuf::FightTeam;

#[test]
fn blocked_ultimate_falls_back_to_the_first_normal_skill() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_skill: Some(103),
                skill_group1: vec![101],
                skill_group2: vec![102],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        player_candidate_pool_with(crate::test_support::game_data(), &fight, |_| true)
            .into_iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![103, 102]
    );
    assert_eq!(
        player_candidate_pool_with(crate::test_support::game_data(), &fight, |_| false)
            .into_iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![101, 102]
    );
}

#[test]
fn normal_cards_do_not_infer_temporary_status_from_owner_uid() {
    let card = card_for(
        &FightEntityInfo {
            uid: Some(-1),
            skill_group1: vec![101],
            ..Default::default()
        },
        Some(101),
    )
    .unwrap();

    assert_eq!(card.temp_card, Some(false));
}

#[test]
fn device_draw_bag_uses_each_owners_selected_device() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    ex_skill_level: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3025),
                    ex_skill_level: Some(3),
                    destiny_stone: Some(302502),
                    destiny_rank: Some(4),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    let cards = device_draw_bag(crate::test_support::game_data(), &fight);
    let count = |uid, hero_id, skill_id| {
        cards
            .iter()
            .filter(|card| {
                card.uid == Some(uid)
                    && card.hero_id == Some(hero_id)
                    && card.skill_id == Some(skill_id)
            })
            .count()
    };

    assert_eq!(cards.len(), 10);
    assert_eq!(count(10, 3144, 31446011), 2);
    assert_eq!(count(10, 3144, 31446012), 2);
    assert_eq!(count(10, 3144, 31447001), 1);
    assert_eq!(count(11, 3025, 31446021), 3);
    assert_eq!(count(11, 3025, 31446022), 1);
    assert_eq!(count(11, 3025, 31447002), 1);
}
