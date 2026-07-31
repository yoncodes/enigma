use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};
use sonettobuf::BuffActInfo;

const RAW_GAUGE_PER_VISIBLE_POINT: i32 = 1_000;

pub fn supports(args: &[i32]) -> bool {
    let [raw_attr_id, value_per_step, raw_cap, rest @ ..] = args else {
        return false;
    };
    AttrId::from_raw(*raw_attr_id).is_some()
        && *value_per_step != 0
        && *raw_cap > 0
        && match rest {
            [] => true,
            [step] => *step > 0,
            _ => false,
        }
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let [act_id, raw_attr_id, ..] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(attr_id) {
        return 0;
    }
    buffs
        .snapshot(feature.owner_uid, feature.buff_uid)
        .and_then(|buff| {
            buff.act_info
                .iter()
                .find(|info| info.act_id == Some(*act_id))
                .and_then(|info| info.param.first())
                .copied()
        })
        .unwrap_or_default()
}

pub fn snapshot(buff_id: i32, visible: i32) -> Option<Vec<BuffActInfo>> {
    let infos = BuffManager::configured_features(buff_id)
        .into_iter()
        .filter(|feature| super::is_kind(feature, super::registry::BuffActKind::AttrByHeatScale))
        .filter_map(|feature| Some((feature.act_id()?, snapshot_delta(&feature, visible)?)))
        .map(|(act_id, delta)| BuffActInfo {
            act_id: Some(act_id),
            param: vec![delta],
            str_param: Some(String::new()),
        })
        .collect::<Vec<_>>();
    (!infos.is_empty()).then_some(infos)
}

fn snapshot_delta(feature: &ActiveBuffFeature, visible: i32) -> Option<i32> {
    let [_, raw_attr_id, value_per_step, raw_cap, rest @ ..] = feature.values.as_slice() else {
        return None;
    };
    let attr_id = AttrId::from_raw(*raw_attr_id)?;
    let raw_step = i64::from(
        rest.first()
            .copied()
            .unwrap_or(RAW_GAUGE_PER_VISIBLE_POINT)
            .max(1),
    );
    let numerator = i64::from(
        visible
            .max(0)
            .saturating_mul(RAW_GAUGE_PER_VISIBLE_POINT)
            .min((*raw_cap).max(0)),
    ) * i64::from(*value_per_step);
    let delta = if attr_id == AttrId::DmgBonus && numerator > 0 {
        (numerator + raw_step - 1) / raw_step
    } else {
        numerator / raw_step
    };
    i32::try_from(delta).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(values: Vec<i32>) -> ActiveBuffFeature {
        ActiveBuffFeature {
            owner_uid: 1,
            source_uid: 1,
            buff_uid: 1,
            buff_id: 1,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "AttrByHeatScale".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: String::new(),
            values,
        }
    }

    #[test]
    fn explicit_step_snapshots_the_scaled_attribute_value() {
        let critical_rate = feature(vec![1053, AttrId::CriticalRate.id(), 5, 1_000_000, 10_000]);
        assert_eq!(snapshot_delta(&critical_rate, 37), Some(18));
    }

    #[test]
    fn damage_bonus_rounds_up_at_the_captured_fractional_boundary() {
        let damage_bonus = feature(vec![1053, AttrId::DmgBonus.id(), 150, 600_000, 100_000]);
        let incantation_might = feature(vec![
            1053,
            AttrId::IncantationMight.id(),
            60,
            600_000,
            100_000,
        ]);
        assert_eq!(snapshot_delta(&damage_bonus, 21), Some(32));
        assert_eq!(snapshot_delta(&incantation_might, 21), Some(12));
    }

    #[test]
    fn configured_heat_scale_buff_snapshots_the_calculated_attribute() {
        crate::test_support::init_config();

        let infos = snapshot(31390175, 21).unwrap();

        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].act_id, Some(1053));
        assert_eq!(infos[0].param, vec![32]);
    }
}
