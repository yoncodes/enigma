use crate::engine::manager::buff::ActiveBuffFeature;

use super::additional_damage::AdditionalDamageSpec;

pub fn resolve(feature: &ActiveBuffFeature) -> Option<AdditionalDamageSpec> {
    if super::feature_kind(feature)
        != Some(super::registry::BuffActKind::CreateMaxHpAdditionalDamageAndRemove)
    {
        return None;
    }
    let (_, args) = feature.values.split_first()?;
    let [source_count_cost, rate, temp_buff_id] = args else {
        return None;
    };
    Some(AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::MaxHpAdditionalDamage,
        rate: *rate,
        secondary_rate: *rate,
        extra_rate: *rate,
        extra_secondary_rate: *rate,
        temp_buff_id: *temp_buff_id,
        remove_buff_id: 0,
        credited_source_uid: feature.owner_uid,
        extra_eureka_cost: 0,
        power_id: 0,
        source_count_cost: *source_count_cost,
        requires_assassination: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_hp_additional_damage_owns_its_rate_and_temporary_calculation_buff() {
        let feature = ActiveBuffFeature {
            owner_uid: 1,
            source_uid: 2,
            buff_uid: 2,
            buff_id: 31260151,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "CreateMaxHpAdditionalDamageAndRemove".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "1026#1#750#31260171".to_owned(),
            values: vec![1026, 1, 750, 31260171],
        };

        assert_eq!(
            resolve(&feature),
            Some(AdditionalDamageSpec {
                formula: crate::engine::damage::DamageFormula::MaxHpAdditionalDamage,
                rate: 750,
                secondary_rate: 750,
                extra_rate: 750,
                extra_secondary_rate: 750,
                temp_buff_id: 31260171,
                remove_buff_id: 0,
                credited_source_uid: 1,
                extra_eureka_cost: 0,
                power_id: 0,
                source_count_cost: 1,
                requires_assassination: false,
            })
        );
    }
}
