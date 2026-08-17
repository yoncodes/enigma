use crate::engine::{
    damage::{DamageFormulaInput, attribute_scaled_damage, calculate_with_trace, modifiers},
    entity::attr::AttrId,
    manager::BattleManagers,
    skill::{effect::ParsedBehavior, target::TargetPool},
};

use super::critical_technique_bonus;

#[derive(Clone, Copy)]
pub(super) struct OriginRuntime<'a> {
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub extra_action: bool,
}

pub(super) fn amount(
    source_uid: i64,
    target_uid: i64,
    runtime: OriginRuntime<'_>,
    attack_attributes: &[(AttrId, i32)],
    is_crit: bool,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let managers = runtime.managers;
    let pool = runtime.pool;
    let [mode, raw_attr, rate] = behavior.args.as_slice() else {
        return None;
    };
    let attr_id = AttrId::from_raw(*raw_attr)?;
    let basis_uid = if *mode == 0 { source_uid } else { target_uid };
    let crit_multiplier = if is_crit {
        let technique = pool
            .entity(source_uid)
            .zip(pool.entity(target_uid))
            .map(|(source, target)| {
                critical_technique_bonus(managers.catalog(), source, target.level, 12)
            })
            .unwrap_or_default();
        let transient_crit = attack_attributes
            .iter()
            .filter_map(|(attr_id, delta)| (*attr_id == AttrId::CriticalDmg).then_some(*delta))
            .sum::<i32>();
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
            + pool.entity(source_uid).map_or(0, |source| {
                modifiers::damage_type_attribute_delta(
                    &managers.buff,
                    &managers.hp,
                    source_uid,
                    source.damage_type,
                    AttrId::CriticalDmg,
                )
            })
            + transient_crit
            - managers.attribute.get(target_uid, AttrId::CriticalDef)
            - modifiers::dynamic_attribute_delta(
                &managers.buff,
                &managers.hp,
                target_uid,
                AttrId::CriticalDef,
            )
            - pool.entity(target_uid).map_or(0, |target| {
                modifiers::damage_type_attribute_delta(
                    &managers.buff,
                    &managers.hp,
                    target_uid,
                    target.damage_type,
                    AttrId::CriticalDef,
                )
            });
        multiplier.max(0)
    } else {
        1000
    };
    let mut formula = DamageFormulaInput::genesis(
        managers.origin_attribute(basis_uid, attr_id),
        *rate,
        genesis_multiplier(managers, source_uid, target_uid, runtime.extra_action),
    );
    formula.crit_multiplier = crit_multiplier;
    formula.is_crit = is_crit;
    let trace = calculate_with_trace(formula);
    if crate::engine::damage::trace::enabled() {
        eprintln!(
            "genesis behavior={} source={source_uid} target={target_uid} basis_uid={basis_uid} attr={attr_id:?} trace={trace:?}",
            behavior.spec.key.opcode,
        );
    }
    Some(trace.amount)
}

pub(super) fn buff_group_amount(
    source_uid: i64,
    target_uid: i64,
    runtime: OriginRuntime<'_>,
    attack_attributes: &[(AttrId, i32)],
    is_crit: bool,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let managers = runtime.managers;
    let [source_mode, raw_attr, rate, buff_group] = behavior.args.as_slice() else {
        return None;
    };
    if *source_mode != 1 || *buff_group <= 0 {
        return None;
    }
    let mut base = behavior.clone();
    base.args = vec![0, *raw_attr, *rate];
    amount(
        source_uid,
        target_uid,
        runtime,
        attack_attributes,
        is_crit,
        &base,
    )
    .map(|amount| {
        attribute_scaled_damage(
            amount,
            1_000,
            managers.buff.buff_group_amount(target_uid, *buff_group),
        )
    })
}

pub(super) fn team_attribute_amount(
    source_uid: i64,
    target_uid: i64,
    runtime: OriginRuntime<'_>,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let managers = runtime.managers;
    let pool = runtime.pool;
    let [source_team, raw_attr, rate] = behavior.args.as_slice() else {
        return None;
    };
    if *source_team != 1 || *rate < 0 {
        return None;
    }
    let attr_id = AttrId::from_raw(*raw_attr)?;
    let basis = pool
        .main_allies(source_uid)
        .iter()
        .map(|entity| i128::from(managers.origin_attribute(entity.uid, attr_id)))
        .sum::<i128>()
        .clamp(0, i128::from(i32::MAX)) as i32;
    let trace = calculate_with_trace(DamageFormulaInput::genesis(
        basis,
        *rate,
        genesis_multiplier(managers, source_uid, target_uid, runtime.extra_action),
    ));
    if crate::engine::damage::trace::enabled() {
        eprintln!(
            "genesis behavior={} source={source_uid} target={target_uid} team_attr={attr_id:?} basis={basis} trace={trace:?}",
            behavior.spec.key.opcode,
        );
    }
    Some(trace.amount)
}

fn genesis_multiplier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    extra_action: bool,
) -> i32 {
    let local = if extra_action {
        managers
            .buff
            .active_features(&managers.hp)
            .iter()
            .filter(|feature| feature.owner_uid == source_uid)
            .map(|feature| {
                crate::engine::skill::buff_act::must_crit_and_fix_temp_attr::attribute_delta(
                    feature,
                    AttrId::GenesisDmgBonus,
                    &managers.attribute,
                    &managers.buff,
                    &managers.hp,
                )
            })
            .sum::<i32>()
    } else {
        0
    };
    modifiers::genesis_multiplier(managers, source_uid, target_uid).saturating_add(local)
}
