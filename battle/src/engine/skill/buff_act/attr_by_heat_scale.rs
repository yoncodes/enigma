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
    snapshot_features(BuffManager::configured_features(buff_id), visible)
}

pub(crate) fn definition_snapshot(
    buffs: &BuffManager,
    buff_id: i32,
    visible: i32,
) -> Option<Vec<BuffActInfo>> {
    snapshot_features(buffs.definition_features(buff_id), visible)
}

fn snapshot_features(features: Vec<ActiveBuffFeature>, visible: i32) -> Option<Vec<BuffActInfo>> {
    let infos = features
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
mod tests;
