use std::collections::HashMap;

use battle::tooling::scan::buff_act::registry::{BuffActKind, resolve_feature};
use sonettobuf::{
    ActEffect, FightRound, FightStep, effect_type_enum::EffectType,
    fight_hurt_info::DamageFromType, fight_step::ActType,
};

type CritKey = (i32, i64, i64);

pub(super) fn choices(game_data: &config::GameDB, round: &FightRound) -> Vec<(CritKey, bool)> {
    let mut observed = HashMap::new();
    for step in &round.fight_step {
        collect(game_data, step, &mut observed);
    }
    observed
        .into_iter()
        .filter_map(|(key, crit)| Some((key, crit?)))
        .collect()
}

fn collect(
    game_data: &config::GameDB,
    step: &FightStep,
    observed: &mut HashMap<CritKey, Option<bool>>,
) {
    for effect in &step.act_effect {
        if let Some((key, is_crit)) = identify(game_data, step, effect) {
            observed
                .entry(key)
                .and_modify(|crit| *crit = None)
                .or_insert(Some(is_crit));
        }
        if let Some(child) = effect.fight_step.as_ref() {
            collect(game_data, child, observed);
        }
    }
}

fn identify(
    game_data: &config::GameDB,
    step: &FightStep,
    effect: &ActEffect,
) -> Option<(CritKey, bool)> {
    if step.act_type != Some(ActType::Effect as i32)
        || effect.config_effect.unwrap_or_default() != 0
    {
        return None;
    }
    let effect_type = effect.effect_type?;
    let is_crit = match effect_type {
        kind if kind == EffectType::Origindamage as i32 => false,
        kind if kind == EffectType::Origincrit as i32 => true,
        _ => return None,
    };
    let carrier_id = step.act_id?;
    let source_uid = step.from_id.filter(|uid| *uid != 0)?;
    let target_uid = effect.target_id.filter(|uid| *uid != 0)?;
    let act_id = effect.buff_act_id?;
    let hurt = effect.hurt_info.as_ref()?;
    if step.to_id != Some(target_uid)
        || hurt.from_uid != Some(source_uid)
        || hurt.buff_act_id != Some(act_id)
        || hurt.buff_uid.unwrap_or_default() <= 0
        || hurt.damage_from_type != Some(DamageFromType::Buff as i32)
        || hurt.hurt_effect != Some(effect_type)
    {
        return None;
    }
    let carrier = game_data.skill_buff.get(carrier_id)?;
    let matches = carrier.features.split('|').any(|raw| {
        resolve_feature(Some(game_data), raw).is_some_and(|feature| {
            feature.arguments_supported
                && feature.act_id == Some(act_id)
                && feature.kind == Some(BuffActKind::TidalTempoSettlement)
        })
    });
    matches.then_some(((carrier_id, source_uid, target_uid), is_crit))
}

#[cfg(test)]
mod tests;
