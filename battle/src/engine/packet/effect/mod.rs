use sonettobuf::{
    ActEffect, BuffActInfo, BuffInfo, CardInfo, EmitterInfo, EnhanceInfoBox, FightEntityInfo,
    FightHurtInfo, FightStep, HeroAttribute, MagicCircleInfo, PowerInfo, SummonedInfo,
    effect_type_enum::EffectType, fight_hurt_info, fight_step,
};

use crate::engine::{
    buff::marker,
    fight::versions::HurtInfoWireLayout,
    manager::{
        buff::{
            BuffApplyResult, BuffMarkerResult, BuffRejectResult, BuffRemoveResult, BuffUpdateResult,
        },
        emitter,
        eureka::EurekaApplyResult,
        ex_point::ExPointApplyResult,
        hp::{DamageRecord, HpChange, HurtDamageFromType, HurtInfoData},
        summon::{SummonApplyResult, summoned_lane},
    },
    mechanic::magic_circle::MagicCircleApplyResult,
};

const SHIELD_VALUE_CHANGE_RESERVE_ID: i64 = 1;

mod buff;

pub struct EffectPacket;

impl EffectPacket {
    pub fn effect_marker(marker: crate::engine::skill::rule::output::EffectMarker) -> ActEffect {
        ActEffect {
            target_id: Some(marker.target_uid),
            effect_type: Some(marker.effect_type),
            effect_num: Some(marker.effect_num),
            config_effect: Some(marker.config_effect),
            reserve_id: marker.reserve_id,
            reserve_str: marker.reserve_str,
            ..Default::default()
        }
    }

    pub fn scene_change(scene_id: i32) -> [ActEffect; 2] {
        [
            ActEffect {
                target_id: Some(0),
                effect_type: Some(EffectType::Fightparamchange as i32),
                effect_num: Some(0),
                reserve_str: Some(format!("16#{scene_id}")),
                ..Default::default()
            },
            ActEffect {
                target_id: Some(0),
                effect_type: Some(EffectType::Changescene as i32),
                effect_num: Some(scene_id),
                ..Default::default()
            },
        ]
    }

    pub fn card_remove(indices: &[usize]) -> ActEffect {
        ActEffect {
            target_id: Some(0),
            effect_type: Some(EffectType::Cardremove as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            reserve_str: Some(
                indices
                    .iter()
                    .map(|index| (index + 1).to_string())
                    .collect::<Vec<_>>()
                    .join("#"),
            ),
            team_type: Some(1),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn card_hand_limit(target_uid: i64, limit: i32, config_effect: i32) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Addcardlimit as i32),
            effect_num: Some(limit),
            config_effect: Some(config_effect),
            ..Default::default()
        }
    }
}

mod buff_info;
mod damage;
mod mechanic;
mod state;
mod step;
mod turn;

fn refresh_increases_effect_value(change: &BuffUpdateResult) -> bool {
    change.before.buff_id != change.after.buff_id
        || change.after.layer.unwrap_or_default() > change.before.layer.unwrap_or_default()
        || change.after.count.unwrap_or_default() > change.before.count.unwrap_or_default()
        || change.before.act_common_params != change.after.act_common_params
        || change.before.act_info != change.after.act_info
}

fn ex_point_effect_type(change: ExPointApplyResult) -> i32 {
    if change.effect_type != 0 {
        return change.effect_type;
    }

    EffectType::Expointchange as i32
}

#[cfg(test)]
mod test;
