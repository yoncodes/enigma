use super::*;

impl EffectPacket {
    pub fn conduit_initialized(area: &crate::engine::manager::conduit::ConduitArea) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Initdevice as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(area.team),
            effect_num1: Some(0),
            device_area_info: Some(sonettobuf::FightDeviceAreaInfo {
                devices: area
                    .devices
                    .iter()
                    .map(|device| sonettobuf::FightDeviceInfo {
                        skills: device
                            .skill_groups
                            .iter()
                            .map(|group| sonettobuf::FightDeviceSkillGroupInfo {
                                skills: group
                                    .iter()
                                    .map(|skill| sonettobuf::FightDeviceSkillInfo {
                                        skill_id: Some(skill.skill_id),
                                        cost_type: Some(skill.cost_type),
                                        cost_value: Some(skill.cost_value),
                                        is_stop: Some(skill.is_stopped),
                                    })
                                    .collect(),
                            })
                            .collect(),
                        index: Some(device.selected_group),
                        uid: Some(device.uid),
                    })
                    .collect(),
                powers: area
                    .powers
                    .iter()
                    .map(|power| sonettobuf::FightDevicePower {
                        id: Some(power.id),
                        power: Some(power.value),
                    })
                    .collect(),
            }),
            ..Default::default()
        }
    }

    pub fn conduit_power_changed(
        team: i32,
        power_id: i32,
        delta: i32,
        kind: crate::engine::manager::conduit::ConduitPowerChangeKind,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Devicepowerchange as i32),
            effect_num: Some(match kind {
                crate::engine::manager::conduit::ConduitPowerChangeKind::Standard => 0,
                crate::engine::manager::conduit::ConduitPowerChangeKind::Interval => 1,
            }),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(format!("{power_id}#{delta}")),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_group_selected(
        source_uid: i64,
        team: i32,
        group: i32,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(source_uid),
            effect_type: Some(EffectType::Deviceskillindex as i32),
            effect_num: Some(group),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_skill_began(team: i32, power_id: i32, spent: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Devicepowerchange as i32),
            effect_num: Some(-1),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(format!("{power_id}#{}", -spent)),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_skill_cost_committed(
        source_uid: i64,
        team: i32,
        consumed_this_round: i32,
    ) -> ActEffect {
        Self::conduit_counter(source_uid, team, 62, consumed_this_round)
    }

    pub fn conduit_powers_cleared(source_uid: i64, team: i32, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: Some(source_uid),
            effect_type: Some(EffectType::Devicepowerclear as i32),
            effect_num: Some(0),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_powers_reset(team: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Devicepowerclear as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_skill_finished(source_uid: i64, team: i32, uses_this_round: i32) -> ActEffect {
        Self::conduit_counter(source_uid, team, 63, uses_this_round)
    }

    pub fn conduit_running(running: bool) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Devicerunning as i32),
            effect_num: Some(i32::from(running)),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    fn conduit_counter(target_uid: i64, team: i32, counter: i32, value: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Counterchange as i32),
            effect_num: Some(counter),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(value.to_string()),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_skill_stopped(source_uid: i64, team: i32, skill_id: i32) -> ActEffect {
        ActEffect {
            target_id: Some(source_uid),
            effect_type: Some(EffectType::Devicestop as i32),
            effect_num: Some(skill_id),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(team),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn conduit_device_restarted(source_uid: i64, team: i32) -> ActEffect {
        Self::conduit_skill_stopped(source_uid, team, 0)
    }

    pub fn blood_pool_max_create(team: i32, config_effect: i32, amount: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Bloodpoolmaxcreate as i32),
            effect_num: Some(team),
            config_effect: Some(config_effect),
            effect_num1: Some(amount),
            ..Default::default()
        }
    }

    pub fn crystal_add_card(card: CardInfo) -> ActEffect {
        ActEffect {
            target_id: card.uid,
            effect_type: Some(EffectType::Crystaladdcard as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(1),
            effect_num1: Some(0),
            card_info: Some(card),
            ..Default::default()
        }
    }

    pub fn butterfly_add_hand_card(card: CardInfo) -> ActEffect {
        ActEffect {
            target_id: card.uid,
            effect_type: Some(EffectType::Butterflyaddhandcard as i32),
            effect_num: card.skill_id,
            card_info: Some(card.clone()),
            card_info_list: vec![card],
            team_type: Some(1),
            ..Default::default()
        }
    }

    pub fn blood_pool_max_change(team: i32, delta: i32) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Bloodpoolmaxchange as i32),
            effect_num: Some(team),
            effect_num1: Some(delta),
            ..Default::default()
        }
    }

    pub fn blood_pool_value_change(
        source_uid: i64,
        team: i32,
        delta: i32,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(source_uid),
            effect_type: Some(EffectType::Bloodpoolvaluechange as i32),
            effect_num: Some(team),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(delta),

            ..Default::default()
        }
    }

    pub fn magic_circle_add(change: &MagicCircleApplyResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Magiccircleadd as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(change.circle_id as i64),
            reserve_str: Some(String::new()),
            team_type: Some(0),
            effect_num1: Some(0),
            magic_circle: Some(MagicCircleInfo {
                magic_circle_id: change.info.magic_circle_id,
                round: change.info.round,
                create_uid: change.info.create_uid,
                electric_level: change.info.electric_level,
                electric_progress: change.info.electric_progress,
                max_electric_progress: change.info.max_electric_progress,
            }),
            ..Default::default()
        }
    }

    pub fn magic_circle_update(change: &MagicCircleApplyResult) -> ActEffect {
        let mut effect = Self::magic_circle_add(change);
        effect.effect_type = Some(EffectType::Magiccircleupdate as i32);
        effect.reserve_str = Some("0".to_owned());
        effect
    }

    pub fn marker(target_uid: i64, effect_type: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(effect_type),
            ..Default::default()
        }
    }

    pub fn summoned_add(change: &SummonApplyResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Summonedadd as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(change.uid),
            reserve_str: Some(String::new()),
            team_type: Some(0),
            effect_num1: Some(0),
            summoned: Some(SummonedInfo {
                summoned_id: Some(change.summoned_id),
                level: Some(change.level),
                uid: Some(change.uid),
                from_uid: Some(change.from_uid),
            }),
            ..Default::default()
        }
    }

    pub fn summoned_level_up(
        change: &SummonApplyResult,
        effect_num: i32,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(change.from_uid),
            effect_type: Some(EffectType::Summonedlevelup as i32),
            effect_num: Some(effect_num),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(change.uid),
            reserve_str: Some(String::new()),
            team_type: Some(0),
            effect_num1: Some(0),
            summoned: Some(SummonedInfo {
                summoned_id: Some(change.summoned_id),
                level: Some(change.level),
                uid: Some(change.uid),
                from_uid: Some(change.from_uid),
            }),
            ..Default::default()
        }
    }

    pub fn summoned_delete(
        source_uid: i64,
        summoned_id: i32,
        count: i32,
        config_effect: i32,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(source_uid),
            effect_type: Some(EffectType::Summoneddelete as i32),
            effect_num: Some(count),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(summoned_lane(summoned_id) as i64),
            team_type: Some(0),
            ..Default::default()
        }
    }

    pub fn notify_upgrade_hero(target_uid: i64, option_id: i32, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Notifyupgradehero as i32),
            effect_num: Some(option_id),
            config_effect: Some(config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn hero_upgrade(target_uid: i64, option_id: i32, entity: FightEntityInfo) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Heroupgrade as i32),
            effect_num: Some(option_id),
            entity: Some(entity),
            ..Default::default()
        }
    }

    pub fn current_hp_change(target_uid: i64, value: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Currenthpchange as i32),
            effect_num: Some(value),
            ..Default::default()
        }
    }

    pub fn max_hp_change(target_uid: i64, value: i32, buff_act_id: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Maxhpchange as i32),
            effect_num: Some(value),
            buff_act_id: Some(buff_act_id),
            ..Default::default()
        }
    }

    pub fn layer_halo_sync(snapshot: &crate::engine::manager::buff::BuffSyncResult) -> ActEffect {
        ActEffect {
            target_id: Some(snapshot.target_uid),
            effect_type: Some(EffectType::Layerhalosync as i32),
            buff: Some(snapshot.buff.clone()),
            ..Default::default()
        }
    }
}
