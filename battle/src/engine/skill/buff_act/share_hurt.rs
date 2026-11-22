use std::collections::HashMap;

use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffConsume, BuffManager, BuffSelector, DepletedBuff},
        entity::EntityManager,
        hp::{HpDamage, HpLoss, HpManager, HurtDamageFromType, HurtInfoData},
    },
    skill::buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
};

/// How one attack hit on a ShareHurt holder is split before it lands.
#[derive(Debug, Clone, PartialEq)]
pub struct SharePlan {
    pub buff_uid: i64,
    pub consume: BuffCommand,
    pub shares: Vec<HpLoss>,
    pub holder_amount: i32,
}

/// Plans the split of an attack hit on a ShareHurt holder: one stack is consumed, every other
/// living main ally loses an equal share, and the holder keeps the same share. `spent` counts
/// stacks already planned for consumption earlier in the same HP batch.
pub fn plan(
    buffs: &BuffManager,
    hp: &HpManager,
    entities: &EntityManager,
    damage: &HpDamage,
    spent: &HashMap<i64, i32>,
) -> Option<SharePlan> {
    // Captures prove the split for skill hits and their skill-effect damage only.
    let hurt = damage.hurt;
    if damage.amount <= 0
        || !matches!(
            hurt.damage_from,
            HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
        )
    {
        return None;
    }
    let feature = buffs.active_features(hp).into_iter().find(|feature| {
        feature.owner_uid == damage.target_uid
            && is_kind(feature, BuffActKind::ShareHurt)
            && feature.amount > spent.get(&feature.buff_uid).copied().unwrap_or_default()
    })?;
    let allies = entities
        .alive_combatants(entities.team_type(damage.target_uid)?, hp)
        .into_iter()
        .filter(|uid| *uid != damage.target_uid)
        .collect::<Vec<_>>();
    if allies.is_empty() {
        return None;
    }
    let share = damage.amount / (allies.len() as i32 + 1);
    let origin = feature_command_origin(&feature)?;
    // Skill hits share with zero effect and skill ids; skill-effect damage keeps its own.
    let (effect_id, skill_id) = if hurt.damage_from == HurtDamageFromType::SkillEffect {
        (hurt.effect_id, hurt.skill_id)
    } else {
        (0, 0)
    };
    let shares = allies
        .into_iter()
        .map(|ally_uid| HpLoss {
            origin,
            source_uid: damage.source_uid,
            target_uid: ally_uid,
            amount: share,
            config_effect: damage.config_effect,
            hurt: Some(HurtInfoData {
                from_uid: hurt.from_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id,
                skill_id,
                damage_from: HurtDamageFromType::ShareHurt,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Sharehurt as i32,
                display_amount: None,
            }),
        })
        .collect();
    Some(SharePlan {
        buff_uid: feature.buff_uid,
        consume: BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: damage.target_uid,
            selector: BuffSelector::Uid(feature.buff_uid),
            // Consume plans store the resulting layer, so count stacks planned earlier in the batch.
            amount: 1 + spent.get(&feature.buff_uid).copied().unwrap_or_default(),
            depleted: DepletedBuff::RemoveWithZeroLayer,
        }),
        shares,
        holder_amount: share,
    })
}

#[cfg(test)]
mod tests;
