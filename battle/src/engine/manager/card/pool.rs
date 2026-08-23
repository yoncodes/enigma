use sonettobuf::{CardInfo, Fight, FightEntityInfo};

use crate::engine::skill::target::TargetEntity;

pub fn player_candidate_pool(game_data: &config::GameDB, fight: &Fight) -> Vec<CardInfo> {
    player_candidate_pool_with(game_data, fight, can_use_ex_skill)
}

pub(super) fn can_use_ex_skill(entity: &FightEntityInfo) -> bool {
    entity.ex_point.unwrap_or_default() >= 5 + entity.expoint_max_add.unwrap_or_default()
}

pub fn player_candidate_pool_with(
    game_data: &config::GameDB,
    fight: &Fight,
    mut can_use_ex_skill: impl FnMut(&FightEntityInfo) -> bool,
) -> Vec<CardInfo> {
    player_candidate_pool_from(fight, &mut can_use_ex_skill, |entity| {
        crate::catalog::configured_device_card_weights(game_data, entity)
    })
}

pub(crate) fn player_candidate_pool_from(
    fight: &Fight,
    mut can_use_ex_skill: impl FnMut(&FightEntityInfo) -> bool,
    configured: impl FnMut(&FightEntityInfo) -> Vec<(i32, usize)>,
) -> Vec<CardInfo> {
    normal_player_candidate_pool_with(fight, &mut can_use_ex_skill)
        .into_iter()
        .chain(device_draw_bag_from(fight, configured))
        .collect()
}

pub(crate) fn normal_player_candidate_pool_with(
    fight: &Fight,
    mut can_use_ex_skill: impl FnMut(&FightEntityInfo) -> bool,
) -> Vec<CardInfo> {
    let Some(attacker) = fight.attacker.as_ref() else {
        return Vec::new();
    };

    let active = main_alive_ordered(&attacker.entitys);
    active
        .iter()
        .filter_map(|entity| card_for(entity, first_skill(entity, can_use_ex_skill(entity))))
        .chain(
            active
                .iter()
                .filter_map(|entity| card_for(entity, entity.skill_group2.first().copied())),
        )
        .collect()
}

pub(crate) fn device_draw_bag(game_data: &config::GameDB, fight: &Fight) -> Vec<CardInfo> {
    device_draw_bag_from(fight, |entity| {
        crate::catalog::configured_device_card_weights(game_data, entity)
    })
}

pub(super) fn device_draw_bag_from(
    fight: &Fight,
    mut configured: impl FnMut(&FightEntityInfo) -> Vec<(i32, usize)>,
) -> Vec<CardInfo> {
    fight
        .attacker
        .iter()
        .flat_map(|team| &team.entitys)
        .flat_map(|entity| {
            let weights = configured(entity);
            weights.into_iter().flat_map(|(skill_id, count)| {
                std::iter::repeat_n(card_for(entity, Some(skill_id)).unwrap(), count)
            })
        })
        .collect()
}

pub fn active_player_uids(fight: &Fight) -> Vec<i64> {
    fight
        .attacker
        .as_ref()
        .map(|team| {
            main_alive_ordered(&team.entitys)
                .into_iter()
                .filter_map(|entity| entity.uid)
                .collect()
        })
        .unwrap_or_default()
}

pub fn active_enemy_entities(fight: &Fight) -> Vec<&FightEntityInfo> {
    fight
        .defender
        .as_ref()
        .map(|team| main_alive_ordered(&team.entitys))
        .unwrap_or_default()
}

pub fn card_for(entity: &FightEntityInfo, skill_id: Option<i32>) -> Option<CardInfo> {
    let skill_id = skill_id.filter(|id| *id > 0)?;
    Some(CardInfo {
        uid: entity.uid,
        skill_id: Some(skill_id),
        card_effect: None,
        temp_card: Some(false),
        enchants: Vec::new(),
        card_type: Some(0),
        hero_id: entity.model_id,
        status: Some(0),
        target_uid: Some(0),
        extra_info: None,
        energy: Some(0),
        extra_infos: Vec::new(),
        area_red_or_blue: Some(0),
        heat_id: Some(0),
        music_note: None,
        card_dataes: Vec::new(),
    })
}

pub fn card_for_target(entity: &TargetEntity, skill_id: i32) -> Option<CardInfo> {
    (skill_id > 0).then(|| CardInfo {
        uid: Some(entity.uid),
        skill_id: Some(skill_id),
        card_effect: None,
        temp_card: Some(false),
        card_type: Some(0),
        hero_id: Some(entity.model_id),
        status: Some(0),
        target_uid: Some(0),
        energy: Some(0),
        area_red_or_blue: Some(0),
        heat_id: Some(0),
        ..Default::default()
    })
}

fn first_skill(entity: &FightEntityInfo, can_use_ex_skill: bool) -> Option<i32> {
    match entity.ex_skill {
        Some(skill_id) if can_use_ex_skill && skill_id > 0 => Some(skill_id),
        _ => entity.skill_group1.first().copied(),
    }
}

fn main_alive_ordered(entities: &[FightEntityInfo]) -> Vec<&FightEntityInfo> {
    let mut active: Vec<_> = entities
        .iter()
        .filter(|entity| entity.current_hp.unwrap_or(1) > 0)
        .collect();
    active.sort_by_key(|entity| entity.position.unwrap_or(i32::MAX));
    active
}

#[cfg(test)]
mod tests {
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
}
