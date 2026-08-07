use super::*;

impl EffectPacket {
    pub fn ex_point(change: ExPointApplyResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(ex_point_effect_type(change)),
            effect_num: Some(change.applied_delta),
            config_effect: Some(change.config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn ex_point_overflow_bank(target_uid: i64, delta: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Expointoverflowbank as i32),
            effect_num: Some(delta),
            config_effect: Some(0),
            ..Default::default()
        }
    }

    pub fn eureka(change: EurekaApplyResult) -> ActEffect {
        let effect_type = if change.effect_type == 0 {
            EffectType::Powerinfochange as i32
        } else {
            change.effect_type
        };
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(effect_type),
            effect_num: Some(change.applied_delta),
            config_effect: Some(change.power_id),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            power_info: (effect_type == EffectType::Powerinfochange as i32).then_some(PowerInfo {
                power_id: Some(change.power_id),
                num: Some(change.after),
                max: Some(change.max),
            }),
            ..Default::default()
        }
    }

    pub fn team_energy_change(team: i32, delta: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Teamenergychange as i32),
            effect_num: Some(team),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(delta),
            ..Default::default()
        }
    }

    pub fn direct_use_ex_skill(target_uid: i64) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Directuseexskill as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn attr_marker(target_uid: i64) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Attr as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn dead(target_uid: i64) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Dead as i32),
            effect_num: Some(0),
            ..Default::default()
        }
    }

    pub fn remove_entity_cards(target_uid: i64, team_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Removeentitycards as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team_type),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn change_hero(defeated_uid: i64, position: i32, entering: FightEntityInfo) -> ActEffect {
        ActEffect {
            target_id: Some(defeated_uid),
            effect_type: Some(EffectType::Changehero as i32),
            effect_num: Some(position),
            entity: Some(entering),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn new_change_wave(fight: sonettobuf::Fight) -> ActEffect {
        ActEffect {
            effect_type: Some(EffectType::Newchangewave as i32),
            fight: Some(fight),
            ..Default::default()
        }
    }

    pub fn monster_change(entity: FightEntityInfo, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: entity.uid,
            effect_type: Some(EffectType::Monsterchange as i32),
            effect_num: entity.model_id,
            entity: Some(entity),
            config_effect: Some(config_effect),
            ..Default::default()
        }
    }

    pub fn monster_summon(
        target_uid: i64,
        entity: FightEntityInfo,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Summon as i32),
            effect_num: entity.model_id,
            entity: Some(entity),
            config_effect: Some(config_effect),
            ..Default::default()
        }
    }

    pub fn kill(target_uid: i64, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Kill as i32),
            effect_num: Some(0),
            config_effect: Some(config_effect),
            ..Default::default()
        }
    }

    pub fn bloodlust(target_uid: i64, amount: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Bloodlust as i32),
            effect_num: Some(amount),
            ..Default::default()
        }
    }

    pub fn shield(target_uid: i64, value: i32, hurt_effect_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Shield as i32),
            effect_num: Some(value),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(hurt_effect_type),
            ..Default::default()
        }
    }

    pub fn shield_value_change(target_uid: i64, value: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Changeshield as i32),
            effect_num: Some(value),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(SHIELD_VALUE_CHANGE_RESERVE_ID),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn shield_delete(target_uid: i64, value: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Shielddel as i32),
            effect_num: Some(value),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn ex_point_max(
        change: crate::engine::manager::ex_point::ExPointMaxApplyResult,
    ) -> ActEffect {
        match change.wire {
            crate::engine::manager::ex_point::ExPointMaxWire::Delta => ActEffect {
                target_id: Some(change.target_uid),
                effect_type: Some(EffectType::Expointmaxadd as i32),
                effect_num: Some(change.applied_delta),
                config_effect: Some(0),
                effect_num1: Some(0),
                ..Default::default()
            },
            crate::engine::manager::ex_point::ExPointMaxWire::Special {
                max_add,
                ultimate_cost_offset,
            } => ActEffect {
                target_id: Some(change.target_uid),
                effect_type: Some(EffectType::Spexpointmaxadd as i32),
                effect_num: Some(0),
                config_effect: Some(0),
                reserve_str: Some(format!("{max_add}#{ultimate_cost_offset}")),
                effect_num1: Some(0),
                ..Default::default()
            },
        }
    }

    pub fn eureka_max_add(target_uid: i64, power_id: i32, delta: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Powermaxadd as i32),
            effect_num: Some(delta),
            config_effect: Some(power_id),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn toughness_recover(
        change: crate::engine::manager::toughness::ToughnessRecovery,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Toughnessrecover as i32),
            effect_num: Some(0),
            config_effect: Some(change.config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(format!("{},{}", change.point, change.value)),
            team_type: Some(change.team_type),
            effect_num1: Some(0),
            ..Default::default()
        }
    }
}
