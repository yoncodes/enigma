use crate::engine::manager::buff::ActiveBuffFeature;

use super::additional_damage::AdditionalDamageSpec;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [rate, 1, 102] if *rate > 0)
}

// "When triggering [Assassination], deals an additional 125% DMG" of the attacker's own type.
pub fn resolve(feature: &ActiveBuffFeature) -> Option<AdditionalDamageSpec> {
    if super::feature_kind(feature)
        != Some(super::registry::BuffActKind::AssassinateCreateAdditionalDamage)
    {
        return None;
    }
    let (_, args) = feature.values.split_first()?;
    let [rate, 1, 102] = args else {
        return None;
    };
    Some(AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        rate: *rate,
        secondary_rate: *rate,
        extra_rate: *rate,
        extra_secondary_rate: *rate,
        temp_buff_id: 0,
        remove_buff_id: 0,
        credited_source_uid: feature.owner_uid,
        extra_eureka_cost: 0,
        power_id: 0,
        source_count_cost: 0,
        requires_assassination: true,
    })
}
