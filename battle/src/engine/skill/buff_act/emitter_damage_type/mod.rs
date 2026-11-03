use crate::engine::{
    manager::{BattleManagers, buff::BuffManager, hp::HpManager},
    skill::target::{EntityDamageType, TargetPool},
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [1])
}

pub fn resolve(
    base: EntityDamageType,
    source_uid: i64,
    buffs: &BuffManager,
    hp: &HpManager,
) -> EntityDamageType {
    if source_uid != crate::engine::manager::emitter::UID {
        return base;
    }
    buffs
        .active_features(hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == source_uid
                && super::is_kind(feature, BuffActKind::EmitterChangeDmgType)
        })
        .find_map(|feature| feature.values.get(1).copied())
        .map(EntityDamageType::from_wire)
        .filter(|damage_type| *damage_type != EntityDamageType::Unknown)
        .unwrap_or(base)
}

pub fn source_type(
    source_uid: i64,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> EntityDamageType {
    pool.entity(source_uid)
        .map(|source| resolve(source.damage_type, source_uid, &managers.buff, &managers.hp))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
