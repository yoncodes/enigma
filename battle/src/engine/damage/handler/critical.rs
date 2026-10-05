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

pub fn heal_multiplier(
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
    let critical_damage = managers.attribute.get(source_uid, AttrId::CriticalDmg)
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
        + pool.entity(source_uid).map_or(0, |source| {
            modifiers::damage_type_attribute_delta(
                &managers.buff,
                &managers.hp,
                source_uid,
                source.damage_type,
                AttrId::CriticalDmg,
            )
        });
    let critical_portion_bonus = managers.buff.buff_act_scalar(
        source_uid,
        crate::engine::skill::buff_act::registry::BuffActKind::HealCritFix,
    );
    heal_multiplier_from_damage_multiplier(critical_damage, critical_portion_bonus)
}

fn heal_multiplier_from_damage_multiplier(
    critical_damage: i32,
    critical_portion_bonus: i32,
) -> i32 {
    const CRITICAL_HEAL_CONVERSION: i32 = 300;

    let critical_portion = (critical_damage - 1000).max(0);
    let conversion = crate::engine::damage::scale_permille(
        CRITICAL_HEAL_CONVERSION,
        1000_i32.saturating_add(critical_portion_bonus),
    );
    let converted = ((i64::from(critical_portion) * i64::from(conversion) + 500) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    1000_i32.saturating_add(converted)
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

#[cfg(test)]
mod tests {
    use super::heal_multiplier_from_damage_multiplier;

    #[test]
    fn critical_healing_converts_thirty_percent_of_the_critical_portion() {
        assert_eq!(heal_multiplier_from_damage_multiplier(1597, 0), 1179);
        assert_eq!(heal_multiplier_from_damage_multiplier(1669, 0), 1201);
    }

    #[test]
    fn heal_crit_fix_increases_only_the_converted_portion() {
        assert_eq!(heal_multiplier_from_damage_multiplier(1500, 500), 1225);
    }
}
