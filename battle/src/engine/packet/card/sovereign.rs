use sonettobuf::{ActEffect, CardInfo, effect_type_enum::EffectType};

use crate::engine::manager::card::SovereignBoostChange;

pub(crate) fn initial_boost(change: &SovereignBoostChange) -> Vec<ActEffect> {
    let destination = i32::try_from(change.to_index + 1).expect("committed boost destination");
    vec![
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Movecard as i32),
            effect_num: Some(i32::try_from(change.from_index + 1).expect("committed boost source")),
            effect_num1: Some(destination),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            ..Default::default()
        },
        data(change.to_index, change.card.clone(), change.team_type),
    ]
}

pub(crate) fn data(index: usize, card: CardInfo, team_type: i32) -> ActEffect {
    ActEffect {
        target_id: Some(0),
        effect_type: Some(EffectType::Updatecarddata as i32),
        effect_num: Some(i32::try_from(index + 1).expect("committed card position")),
        effect_num1: Some(0),
        config_effect: Some(0),
        buff_act_id: Some(0),
        reserve_id: Some(0),
        team_type: Some(team_type),
        card_info: Some(super::wire_card(card)),
        ..Default::default()
    }
}
