use super::*;

impl EffectPacket {
    pub(crate) fn hp_with_hurt_info_layout(
        change: HpChange,
        hurt_info_layout: HurtInfoWireLayout,
    ) -> ActEffect {
        Self::hp_with_hurt_info_and_toughness_layout(change, None, hurt_info_layout)
    }

    pub(crate) fn hp_with_hurt_info_and_toughness_layout(
        change: HpChange,
        toughness: Option<crate::engine::manager::toughness::ToughnessChange>,
        hurt_info_layout: HurtInfoWireLayout,
    ) -> ActEffect {
        let effect_type = if change.effect_type != 0 {
            change.effect_type
        } else if let Some(hurt) = change.hurt {
            if hurt.hurt_effect_type != 0 {
                hurt.hurt_effect_type
            } else if hurt.is_crit {
                EffectType::Crit as i32
            } else {
                EffectType::Damage as i32
            }
        } else if change.hurt.is_none()
            && (change.delta > 0 || change.display_amount.is_some_and(|amount| amount > 0))
        {
            EffectType::Heal as i32
        } else {
            EffectType::Damage as i32
        };

        let display_amount = change
            .display_amount
            .or_else(|| change.hurt.and_then(|hurt| hurt.display_amount))
            .unwrap_or_else(|| change.delta.abs());

        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(effect_type),
            effect_num: Some(display_amount),
            config_effect: Some(change.config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            hurt_info: change.hurt.map(|hurt| {
                Self::hurt_info(change, hurt, toughness, effect_type, hurt_info_layout)
            }),
            ..Default::default()
        }
    }

    pub(crate) fn fully_absorbed_damage_with_toughness_layout(
        target_uid: i64,
        damage: DamageRecord,
        toughness: Option<crate::engine::manager::toughness::ToughnessChange>,
        hurt_info_layout: HurtInfoWireLayout,
    ) -> ActEffect {
        let mut hurt = damage.hurt;
        hurt.display_amount = Some(match hurt_info_layout {
            HurtInfoWireLayout::Version6 => 0,
            HurtInfoWireLayout::Version7 => damage.amount,
        });
        let mut effect = Self::hp_with_hurt_info_and_toughness_layout(
            HpChange {
                target_uid,
                before: 0,
                delta: 0,
                after: 0,
                max: 0,
                config_effect: damage.config_effect,
                hurt: Some(hurt),
                assassinate: damage.assassinate,
                effect_type: 0,
                display_amount: Some(0),
            },
            toughness,
            hurt_info_layout,
        );
        if hurt.damage_from == HurtDamageFromType::Buff {
            effect.buff_act_id = Some(hurt.buff_act_id);
        }
        effect
    }

    pub fn nuo_di_ka_hit(hit: crate::engine::mechanic::nuo_di_ka::NuoDiKaHit) -> ActEffect {
        ActEffect {
            target_id: Some(hit.target_uid),
            effect_type: Some(if hit.mass {
                EffectType::Nuodikateamattack as i32
            } else {
                EffectType::Nuodikarandomattack as i32
            }),
            effect_num: Some(hit.amount.max(0)),
            config_effect: Some(hit.config_effect),
            buff_act_id: Some(hit.buff_act_id),
            reserve_id: Some(0),
            reserve_str: Some(if hit.mass {
                String::new()
            } else {
                format!("{}#{}", hit.hit_index, hit.points)
            }),
            team_type: Some(0),
            effect_num1: Some(match hit.effect_kind {
                crate::engine::manager::hp::DamageEffectKind::Critical => EffectType::Crit as i32,
                _ => EffectType::Damage as i32,
            }),
            ..Default::default()
        }
    }

    pub fn nuo_di_ka_channel(
        change: crate::engine::mechanic::nuo_di_ka::NuoDiKaChange,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(change.owner_uid),
            effect_type: Some(EffectType::Nuodikarandomattacknum as i32),
            effect_num: Some(change.after.points),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(String::new()),
            team_type: Some(0),
            effect_num1: Some(i32::from(change.active)),
            ..Default::default()
        }
    }

    fn hurt_info(
        change: HpChange,
        hurt: HurtInfoData,
        toughness: Option<crate::engine::manager::toughness::ToughnessChange>,
        effect_type: i32,
        layout: HurtInfoWireLayout,
    ) -> FightHurtInfo {
        let (effect_id, skill_id) = if hurt.damage_from == HurtDamageFromType::Skill {
            (0, 0)
        } else {
            (hurt.effect_id, hurt.skill_id)
        };
        let common = FightHurtInfo {
            damage: Some(hurt.display_amount.unwrap_or_else(|| change.delta.abs())),
            reduce_hp: Some(hurt.reduce_hp),
            career_restraint: Some(hurt.career_restraint),
            assassinate: Some(change.assassinate),
            hurt_effect: Some(effect_type),
            damage_from_type: Some(match hurt.damage_from {
                HurtDamageFromType::None => fight_hurt_info::DamageFromType::None as i32,
                HurtDamageFromType::Skill => fight_hurt_info::DamageFromType::Skill as i32,
                HurtDamageFromType::SkillEffect => {
                    fight_hurt_info::DamageFromType::SkillEffect as i32
                }
                HurtDamageFromType::Buff => fight_hurt_info::DamageFromType::Buff as i32,
                HurtDamageFromType::Additional => {
                    fight_hurt_info::DamageFromType::Additional as i32
                }
                HurtDamageFromType::AbsorbHurt => {
                    fight_hurt_info::DamageFromType::AbsorbHurt as i32
                }
                HurtDamageFromType::ShareHurt => fight_hurt_info::DamageFromType::ShareHurt as i32,
            }),
            config_effect: Some(change.config_effect),
            buff_act_id: Some(hurt.buff_act_id),
            buff_uid: Some(hurt.buff_uid as i32),
            effect_id: Some(effect_id),
            skill_id: Some(skill_id),
            from_uid: Some(hurt.from_uid),
            ..Default::default()
        };
        match layout {
            HurtInfoWireLayout::Version6 => FightHurtInfo {
                reduce_shield: Some(0),
                ..common
            },
            HurtInfoWireLayout::Version7 => FightHurtInfo {
                toughness_value: Some(toughness.map_or(0, |change| change.value_delta)),
                toughness_point: Some(toughness.map_or(0, |change| change.point_delta)),
                broken: Some(toughness.is_some_and(|change| change.broke)),
                absorb_hurt_param: Some(
                    r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#
                        .into(),
                ),
                hurt_merge_flag: Some(0),
                ..common
            },
        }
    }

    pub(crate) fn damage_by_buff_act_with_hurt_info_layout(
        change: HpChange,
        buff_act_id: i32,
        hurt_info_layout: HurtInfoWireLayout,
    ) -> ActEffect {
        ActEffect {
            buff_act_id: Some(buff_act_id),
            ..Self::hp_with_hurt_info_layout(change, hurt_info_layout)
        }
    }
}
