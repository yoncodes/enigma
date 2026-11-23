use sonettobuf::{ActEffect, FightStep, fight_step};

use super::normalize_effect_tree;

pub struct StepPacket;

impl StepPacket {
    pub fn effect(effects: Vec<ActEffect>) -> Option<FightStep> {
        if effects.is_empty() {
            return None;
        }

        Some(FightStep {
            act_type: Some(fight_step::ActType::Effect as i32),
            from_id: Some(0),
            to_id: Some(0),
            act_id: Some(0),
            act_effect: effects.into_iter().map(normalize_effect_tree).collect(),
            card_index: Some(0),
            support_hero_id: None,
            fake_timeline: Some(false),
            real_skill_type: Some(0),
            real_skin_id: Some(0),
        })
    }
}
