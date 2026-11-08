use crate::engine::manager::{
    buff::{ActiveBuffFeature, BuffManager},
    emitter,
    hp::HpManager,
};

use super::{is_kind, registry::BuffActKind};

const BASE_ATTACKS: i32 = 1;
const DEFAULT_SPLIT_DAMAGE_REDUCTION: i32 = 700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub count: i32,
    pub damage_rate: i32,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [count] if *count > 0)
}

pub fn supports_split(args: &[i32]) -> bool {
    matches!(args, [start, interval, count] if *start > 0 && *interval > 0 && *count > 0)
}

pub fn supports_limit(args: &[i32]) -> bool {
    matches!(args, [limit, rate, maximum] if *limit > 0 && *rate > 0 && *maximum > 0)
}

pub fn supports_reduction(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn attack_count(buffs: &BuffManager, hp: &HpManager) -> i32 {
    attack_count_for(buffs, hp, emitter::UID)
}

pub fn attack_count_for(buffs: &BuffManager, hp: &HpManager, emitter_uid: i64) -> i32 {
    resolve(buffs, hp, emitter_uid).count
}

pub fn resolve(buffs: &BuffManager, hp: &HpManager, emitter_uid: i64) -> Resolution {
    let features = buffs.active_features(hp);
    let raw_count = BASE_ATTACKS
        + features
            .iter()
            .filter(|feature| feature.owner_uid == emitter_uid)
            .map(attack_count_delta)
            .sum::<i32>();
    let mut count = raw_count.max(BASE_ATTACKS);
    let mut damage_rate: i32 = 0;
    for (limit, rate) in features
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .filter_map(|feature| attack_limit(feature, raw_count))
    {
        count = count.min(limit);
        damage_rate = damage_rate.saturating_add(rate);
    }

    Resolution { count, damage_rate }
}

fn attack_limit(feature: &ActiveBuffFeature, raw_count: i32) -> Option<(i32, i32)> {
    let [_, limit, rate, maximum, ..] = feature.values.as_slice() else {
        return None;
    };
    if !is_kind(feature, BuffActKind::EmitterAttackNumLimitFixDamage)
        || *limit <= 0
        || *rate <= 0
        || *maximum <= 0
    {
        return None;
    }
    let excess = raw_count.saturating_sub(*limit).min(*maximum);
    Some((*limit, rate.saturating_mul(excess)))
}

pub fn split_count(buffs: &BuffManager, hp: &HpManager, attack_index: i32) -> i32 {
    split_count_for(buffs, hp, emitter::UID, attack_index)
}

pub fn split_count_for(
    buffs: &BuffManager,
    hp: &HpManager,
    emitter_uid: i64,
    attack_index: i32,
) -> i32 {
    let features = buffs.active_features(hp);
    let split_count = features
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .map(|feature| split_count_on_attack(feature, attack_index))
        .sum::<i32>();
    if split_count == 0 {
        return 0;
    }

    split_count
        + features
            .iter()
            .filter(|feature| feature.owner_uid == emitter_uid)
            .map(additional_split_count)
            .sum::<i32>()
}

pub fn split_final_damage_delta(buffs: &BuffManager, hp: &HpManager, split_count: i32) -> i32 {
    split_final_damage_delta_for(buffs, hp, emitter::UID, split_count)
}

pub fn split_final_damage_delta_for(
    buffs: &BuffManager,
    hp: &HpManager,
    emitter_uid: i64,
    split_count: i32,
) -> i32 {
    if split_count <= 0 {
        return 0;
    }
    let features = buffs.active_features(hp);
    let configured_fix = features
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .filter(|feature| is_kind(feature, BuffActKind::EmitterFixSubTargetsDamageReduceRate))
        .filter_map(|feature| feature.values.get(1))
        .sum::<i32>();
    -(DEFAULT_SPLIT_DAMAGE_REDUCTION - configured_fix).max(0)
}

fn attack_count_delta(feature: &ActiveBuffFeature) -> i32 {
    if !is_kind(feature, BuffActKind::EmitterNumChange) {
        return 0;
    }
    feature.values.get(1).copied().unwrap_or_default().max(0) * feature.amount.max(1)
}

fn split_count_on_attack(feature: &ActiveBuffFeature, attack_index: i32) -> i32 {
    let [_, start, interval, count, ..] = feature.values.as_slice() else {
        return 0;
    };
    if !is_kind(feature, BuffActKind::AttackNumSplitEmitterNum)
        || *count <= 0
        || *interval <= 0
        || attack_index < *start
        || (attack_index - start) % interval != 0
    {
        return 0;
    }
    count * feature.amount.max(1)
}

fn additional_split_count(feature: &ActiveBuffFeature) -> i32 {
    if !is_kind(feature, BuffActKind::AddSplitEmitterNum) {
        return 0;
    }
    feature.values.get(1).copied().unwrap_or_default().max(0) * feature.amount.max(1)
}

#[cfg(test)]
mod tests;
