use std::collections::HashSet;

use crate::{
    catalog::BattleCatalog,
    engine::{
        damage::{DamageRateComposition, DamageRateTerm},
        manager::{
            buff::{ActiveBuffFeature, BuffManager},
            hp::HpManager,
        },
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [value] if *value != 0)
}

pub fn skill_rate_bonus(feature: &ActiveBuffFeature) -> i32 {
    values_bonus(&feature.values, feature.amount)
}

pub fn buff_id_skill_rate_bonus(buff_id: i32) -> i32 {
    BattleCatalog::try_global()
        .map(|catalog| resolve_buff_id_skill_rate_bonus(catalog, buff_id))
        .unwrap_or_default()
}

fn resolve_buff_id_skill_rate_bonus(catalog: BattleCatalog, buff_id: i32) -> i32 {
    catalog
        .buff_feature_rows(buff_id)
        .into_iter()
        .map(|raw| {
            raw.split('#')
                .filter_map(|value| value.parse::<i32>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|values| {
            values
                .first()
                .and_then(|act_id| catalog.buff_act_definition(*act_id))
                .map(|definition| definition.kind)
                == Some(super::registry::BuffActKind::LifeAttackFixRate)
        })
        .map(|values| values_bonus(&values, 1))
        .sum()
}

pub fn active_damage_rate_terms(
    source_uid: i64,
    buffs: &BuffManager,
    hp: &HpManager,
) -> Vec<DamageRateTerm> {
    let features = buffs.active_features(hp);
    let linked_sources = features
        .iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .filter(|feature| {
            super::is_kind(
                feature,
                super::registry::BuffActKind::CreateMaxHpAdditionalDamageAndRemove,
            )
        })
        .map(|feature| feature.source_uid)
        .filter(|linked_uid| *linked_uid != 0 && *linked_uid != source_uid)
        .collect::<HashSet<_>>();

    features
        .into_iter()
        .filter(|feature| {
            feature.owner_alive
                && (feature.owner_uid == source_uid || linked_sources.contains(&feature.owner_uid))
        })
        .filter(|feature| super::is_kind(feature, super::registry::BuffActKind::LifeAttackFixRate))
        .filter_map(|feature| {
            let rate = skill_rate_bonus(&feature);
            (rate != 0).then(|| DamageRateTerm {
                opcode: feature.values[0],
                rate,
                career_scaled: true,
                composition: if feature.owner_uid == source_uid {
                    DamageRateComposition::RetributionLane
                } else {
                    DamageRateComposition::ProducerMultiplier
                },
            })
        })
        .collect()
}

fn values_bonus(values: &[i32], amount: i32) -> i32 {
    match values {
        [_, value] => value.saturating_mul(amount),
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
