use crate::engine::{
    entity::attr::AttrId,
    manager::{buff::ActiveBuffFeature, hp::HpManager},
};

use super::additional_damage::AdditionalDamageSpec;

pub fn additional_damage(feature: &ActiveBuffFeature) -> Option<AdditionalDamageSpec> {
    if super::feature_kind(feature)
        != Some(super::registry::BuffActKind::AttrOnlyCalDamageReplaceAttrAdCreator)
    {
        return None;
    }
    let [_, target_attr, source_attr, _] = feature.values.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*target_attr) != Some(AttrId::Attack)
        || AttrId::from_raw(*source_attr) != Some(AttrId::Hp)
    {
        return None;
    }
    Some(AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::AttributeReplacementAdditional,
        rate: 1000,
        secondary_rate: 1000,
        extra_rate: 1000,
        extra_secondary_rate: 1000,
        temp_buff_id: 0,
        remove_buff_id: feature.buff_id,
        credited_source_uid: feature.owner_uid,
        extra_eureka_cost: 0,
        power_id: 0,
        source_count_cost: 0,
        requires_assassination: false,
    })
}

pub fn attack_replacement(feature: &ActiveBuffFeature, hp: &HpManager) -> Option<i32> {
    replacement(feature, hp).map(|(_, _, amount)| amount)
}

fn replacement(feature: &ActiveBuffFeature, hp: &HpManager) -> Option<(AttrId, AttrId, i32)> {
    match feature.values.as_slice() {
        [_, target_attr, source_attr, rate]
            if AttrId::from_raw(*target_attr) == Some(AttrId::Attack)
                && AttrId::from_raw(*source_attr) == Some(AttrId::Hp) =>
        {
            Some((
                AttrId::Attack,
                AttrId::Hp,
                hp.max(feature.owner_uid).max(0) * (*rate).max(0) / 1000,
            ))
        }
        _ => None,
    }
}

pub fn additional_damage_attack_replacement(
    feature: &ActiveBuffFeature,
    hp: &HpManager,
) -> Option<super::AttackReplacement> {
    replacement(feature, hp).map(
        |(replaced_attr, source_attr, amount)| super::AttackReplacement {
            replaced_attr,
            source_attr,
            amount,
            formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        },
    )
}

pub fn skill_attack_replacement(
    feature: &ActiveBuffFeature,
    hp: &HpManager,
) -> Option<super::AttackReplacement> {
    replacement(feature, hp).map(
        |(replaced_attr, source_attr, amount)| super::AttackReplacement {
            replaced_attr,
            source_attr,
            amount,
            formula: crate::engine::damage::DamageFormula::AttributeReplacement,
        },
    )
}

#[cfg(test)]
mod tests;
