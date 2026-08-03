use crate::engine::{
    entity::attr::AttrId,
    manager::{BattleManagers, buff::BuffManager, hp::HpManager},
    skill::{
        buff_act::{self, registry::BuffActKind},
        target::EntityDamageType,
    },
};

pub fn dynamic_attribute_delta(
    buffs: &BuffManager,
    hp: &HpManager,
    uid: i64,
    attr_id: AttrId,
) -> i32 {
    persistent_attribute_delta(buffs, hp, uid, attr_id)
        + buffs
            .active_features(hp)
            .iter()
            .filter(|feature| feature.owner_uid == uid)
            .map(|feature| buff_act::attack_attribute_delta(feature, attr_id, buffs, hp))
            .sum::<i32>()
}

pub fn damage_type_attribute_delta(
    buffs: &BuffManager,
    hp: &HpManager,
    uid: i64,
    damage_type: EntityDamageType,
    attr_id: AttrId,
) -> i32 {
    buffs
        .active_features(hp)
        .iter()
        .filter(|feature| feature.owner_uid == uid)
        .map(|feature| {
            buff_act::attr_by_damage_type::attribute_delta(feature, damage_type, attr_id)
        })
        .sum()
}

pub fn persistent_attribute_delta(
    buffs: &BuffManager,
    hp: &HpManager,
    uid: i64,
    attr_id: AttrId,
) -> i32 {
    crate::engine::manager::persistent_attribute_delta(buffs, hp, uid, attr_id)
}

pub fn genesis_multiplier(managers: &BattleManagers, source_uid: i64, target_uid: i64) -> i32 {
    genesis_multiplier_with(
        managers,
        source_uid,
        target_uid,
        dynamic_attribute_delta,
        "genesis",
    )
}

pub fn periodic_genesis_multiplier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
) -> i32 {
    genesis_multiplier_with(
        managers,
        source_uid,
        target_uid,
        persistent_attribute_delta,
        "periodic genesis",
    )
}

fn genesis_multiplier_with(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    attribute_delta: fn(&BuffManager, &HpManager, i64, AttrId) -> i32,
    trace_label: &str,
) -> i32 {
    let base = managers.attribute.get(source_uid, AttrId::GenesisDmgBonus);
    let buffs = attribute_delta(
        &managers.buff,
        &managers.hp,
        source_uid,
        AttrId::GenesisDmgBonus,
    );
    let dealt = managers
        .buff
        .buff_act_scalar(source_uid, BuffActKind::RealHarmFix);
    let taken = managers
        .buff
        .buff_act_scalar(target_uid, BuffActKind::RealHurtFix);
    let multiplier = (1000 + base + buffs + dealt + taken).max(0);
    if super::trace::enabled() {
        eprintln!(
            "{trace_label} modifiers source={source_uid} target={target_uid} base={base} buffs={buffs} dealt={dealt} taken={taken} multiplier={multiplier}"
        );
    }
    multiplier
}
