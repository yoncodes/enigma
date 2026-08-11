use super::*;

impl EffectPacket {
    pub fn from_fight_step(step: FightStep) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Fightstep as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            fight_step: Some(step),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn effect_fight_step_from(from_uid: i64, effects: Vec<ActEffect>) -> ActEffect {
        Self::nested_step(
            fight_step::ActType::Effect as i32,
            from_uid,
            0,
            0,
            0,
            effects,
        )
    }

    pub fn effect_fight_step_action(
        from_uid: i64,
        to_uid: i64,
        act_id: i32,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        Self::nested_step(
            fight_step::ActType::Effect as i32,
            from_uid,
            to_uid,
            act_id,
            0,
            effects,
        )
    }

    pub fn skill_fight_step(
        act_id: i32,
        from_uid: i64,
        to_uid: i64,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        Self::skill_fight_step_with_card_index(act_id, from_uid, to_uid, 0, effects)
    }

    pub fn skill_fight_step_with_card_index(
        act_id: i32,
        from_uid: i64,
        to_uid: i64,
        card_index: i32,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        Self::nested_step(
            fight_step::ActType::Skill as i32,
            from_uid,
            to_uid,
            act_id,
            card_index,
            effects,
        )
    }

    pub fn conduit_skill_fight_step(
        act_id: i32,
        from_uid: i64,
        to_uid: i64,
        card_index: i32,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        Self::nested_step(
            fight_step::ActType::Device as i32,
            from_uid,
            to_uid,
            act_id,
            card_index,
            effects,
        )
    }

    pub fn conduit_fight_step(
        source_uid: i64,
        target_uid: i64,
        group: i32,
        skill_position: i32,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        let mut effect = Self::nested_step(
            fight_step::ActType::Device as i32,
            source_uid,
            target_uid,
            0,
            group,
            effects,
        );
        if let Some(step) = effect.fight_step.as_mut() {
            step.support_hero_id = Some(skill_position);
        }
        effect
    }

    pub(super) fn nested_step(
        act_type: i32,
        from_uid: i64,
        to_uid: i64,
        act_id: i32,
        card_index: i32,
        effects: Vec<ActEffect>,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Fightstep as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            fight_step: Some(FightStep {
                act_type: Some(act_type),
                from_id: Some(from_uid),
                to_id: Some(to_uid),
                act_id: Some(act_id),
                act_effect: effects,
                card_index: Some(card_index),
                support_hero_id: Some(0),
                fake_timeline: Some(false),
                real_skill_type: Some(0),
                real_skin_id: Some(0),
            }),
            effect_num1: Some(0),
            ..Default::default()
        }
    }
}
