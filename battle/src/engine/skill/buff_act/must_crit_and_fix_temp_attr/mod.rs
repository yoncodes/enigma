use crate::engine::{
    damage::modifiers,
    entity::attr::AttrId,
    manager::{
        attribute::AttributeManager,
        buff::{ActiveBuffFeature, BuffManager},
        hp::HpManager,
    },
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [output, critical_rate, critical_damage]
            if matches!(
                AttrId::from_raw(*output),
                Some(AttrId::DmgBonus | AttrId::GenesisDmgBonus)
            )
                && AttrId::from_raw(*critical_rate) == Some(AttrId::CriticalRate)
                && AttrId::from_raw(*critical_damage) == Some(AttrId::CriticalDmg)
    )
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    attributes: &AttributeManager,
    buffs: &BuffManager,
    hp: &HpManager,
) -> i32 {
    if !supports(feature.values.get(1..).unwrap_or_default()) {
        return 0;
    }
    let [_, output, critical_rate, critical_damage] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*output) != Some(attr_id) {
        return 0;
    }
    let Some(critical_rate) = AttrId::from_raw(*critical_rate) else {
        return 0;
    };
    let Some(critical_damage) = AttrId::from_raw(*critical_damage) else {
        return 0;
    };
    let owner_uid = feature.owner_uid;
    let rate = attributes.get(owner_uid, critical_rate)
        + modifiers::persistent_attribute_delta(buffs, hp, owner_uid, critical_rate);
    let damage = attributes.get(owner_uid, critical_damage)
        + modifiers::persistent_attribute_delta(buffs, hp, owner_uid, critical_damage);

    (i64::from(rate.max(0)) * i64::from((damage - 1000).max(0)) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32
}

pub fn forces_critical(
    features: &[ActiveBuffFeature],
    source_uid: i64,
    extra_action: bool,
) -> bool {
    extra_action
        && features.iter().any(|feature| {
            feature.owner_uid == source_uid
                && feature.owner_alive
                && feature.amount > 0
                && super::is_kind(feature, BuffActKind::MustCritAndFixTempAttr)
                && supports(feature.values.get(1..).unwrap_or_default())
        })
}

pub fn active_forces_critical(
    source_uid: i64,
    managers: &crate::engine::manager::BattleManagers,
    extra_action: bool,
) -> bool {
    extra_action
        && forces_critical(
            &managers.buff.active_features(&managers.hp),
            source_uid,
            true,
        )
}

#[cfg(test)]
mod tests;
