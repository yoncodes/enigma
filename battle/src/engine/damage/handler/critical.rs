use crate::engine::{
    damage::modifiers,
    entity::attr::AttrId,
    manager::{BattleManagers, buff::BuffManager},
    skill::target::TargetPool,
};

use super::critical_technique_bonus;

pub fn chance(
    source_uid: i64,
    target_uid: i64,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> i32 {
    raw_chance(source_uid, target_uid, pool, managers, &managers.buff).clamp(0, 1000)
}

pub fn damage_multiplier(
    source_uid: i64,
    target_uid: i64,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> i32 {
    let technique = pool
        .entity(source_uid)
        .zip(pool.entity(target_uid))
        .map(|(source, target)| {
            critical_technique_bonus(managers.catalog(), source, target.level, 12)
        })
        .unwrap_or_default();
    let multiplier = managers.attribute.get(source_uid, AttrId::CriticalDmg)
        + managers
            .buff
            .fixed_attribute_delta(source_uid, AttrId::CriticalDmg)
        + technique
        + modifiers::dynamic_attribute_delta(
            &managers.buff,
            &managers.hp,
            source_uid,
            AttrId::CriticalDmg,
        )
        + modifiers::damage_type_attribute_delta(
            &managers.buff,
            &managers.hp,
            source_uid,
            pool.entity(source_uid)
                .map(|entity| entity.damage_type)
                .unwrap_or_default(),
            AttrId::CriticalDmg,
        )
        - managers.attribute.get(target_uid, AttrId::CriticalDef)
        - modifiers::dynamic_attribute_delta(
            &managers.buff,
            &managers.hp,
            target_uid,
            AttrId::CriticalDef,
        )
        - modifiers::damage_type_attribute_delta(
            &managers.buff,
            &managers.hp,
            target_uid,
            pool.entity(target_uid)
                .map(|entity| entity.damage_type)
                .unwrap_or_default(),
            AttrId::CriticalDef,
        );
    multiplier.max(0)
}

fn raw_chance(
    source_uid: i64,
    target_uid: i64,
    pool: &TargetPool,
    managers: &BattleManagers,
    target_buffs: &BuffManager,
) -> i32 {
    let Some(source) = pool.entity(source_uid) else {
        return 0;
    };
    let Some(target) = pool.entity(target_uid) else {
        return 0;
    };
    let emitter_attribute = if source_uid == crate::engine::manager::emitter::UID {
        crate::engine::manager::emitter::average_ally_buff_attribute(
            pool,
            &managers.buff,
            &managers.hp,
            AttrId::CriticalRate,
        ) + managers.emitter.ally_attribute(AttrId::CriticalRate)
    } else {
        0
    };
    managers.attribute.get(source_uid, AttrId::CriticalRate)
        + critical_technique_bonus(managers.catalog(), source, target.level, 11)
        + modifiers::dynamic_attribute_delta(
            &managers.buff,
            &managers.hp,
            source_uid,
            AttrId::CriticalRate,
        )
        + modifiers::damage_type_attribute_delta(
            &managers.buff,
            &managers.hp,
            source_uid,
            source.damage_type,
            AttrId::CriticalRate,
        )
        + emitter_attribute
        - managers
            .attribute
            .get(target_uid, AttrId::CriticalResistRate)
        - modifiers::dynamic_attribute_delta(
            target_buffs,
            &managers.hp,
            target_uid,
            AttrId::CriticalResistRate,
        )
        - modifiers::damage_type_attribute_delta(
            target_buffs,
            &managers.hp,
            target_uid,
            target.damage_type,
            AttrId::CriticalResistRate,
        )
}

/// Returns the current attack's critical chance above 100%.
pub fn excess_rate(
    source_uid: i64,
    target_uid: i64,
    pool: &TargetPool,
    managers: &BattleManagers,
    attack_attributes: &[(AttrId, i32)],
) -> i32 {
    let attack_local = attack_attributes
        .iter()
        .filter(|(attr_id, _)| *attr_id == AttrId::CriticalRate)
        .map(|(_, delta)| *delta)
        .fold(0, i32::saturating_add);
    let raw = raw_chance(source_uid, target_uid, pool, managers, &managers.buff)
        .saturating_add(attack_local);
    (raw - 1000).max(0)
}
