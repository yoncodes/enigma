use crate::engine::manager::buff::ActiveBuffFeature;

use super::additional_damage::AdditionalDamageSpec;

pub fn resolve(feature: &ActiveBuffFeature) -> Option<AdditionalDamageSpec> {
    if super::feature_kind(feature) != Some(super::registry::BuffActKind::CreateAdditionalDamage) {
        return None;
    }
    let (_, args) = feature.values.split_first()?;
    let [
        cost,
        rate,
        secondary_rate,
        extra_rate,
        extra_secondary_rate,
        power_id,
        102,
    ] = args
    else {
        return None;
    };
    Some(AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        rate: *rate,
        secondary_rate: *secondary_rate,
        extra_rate: *extra_rate,
        extra_secondary_rate: *extra_secondary_rate,
        temp_buff_id: 0,
        remove_buff_id: 0,
        credited_source_uid: feature.source_uid,
        extra_eureka_cost: *cost,
        power_id: *power_id,
        source_count_cost: 0,
        requires_assassination: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_additional_damage_is_credited_to_the_feature_source() {
        let feature = ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 20,
            buff_uid: 2,
            buff_id: 31050144,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "CreateAdditionalDamage".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "863#2#300#150#600#300#1#102".to_owned(),
            values: vec![863, 2, 300, 150, 600, 300, 1, 102],
        };

        assert_eq!(resolve(&feature).unwrap().credited_source_uid, 20);
    }
}
