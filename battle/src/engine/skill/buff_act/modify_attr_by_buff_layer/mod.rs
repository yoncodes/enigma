use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [tracked_buff_id, 1, raw_attr, per_layer, max_layers]
            if *tracked_buff_id > 0
                && AttrId::from_raw(*raw_attr).is_some()
                && *per_layer > 0
                && *max_layers > 0
    )
}

pub(crate) fn grant_params(
    act_id: i32,
    args: &[i32],
    source_uid: i64,
    buffs: &BuffManager,
) -> Option<String> {
    if !supports(args) {
        return None;
    }
    let [tracked_buff_id, _, _, per_layer, max_layers] = args else {
        return None;
    };
    let value = buffs
        .buff_id_or_type_amount(source_uid, *tracked_buff_id)
        .clamp(0, *max_layers)
        .saturating_mul(*per_layer);
    Some(format!("{act_id}#{}.{}", value / 10, value % 10))
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let [act_id, _, 1, raw_attr, _, _] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr) != Some(attr_id) {
        return 0;
    }
    buffs
        .act_common_params(feature.owner_uid, feature.buff_uid)
        .and_then(|params| read_value(params, *act_id))
        .unwrap_or_default()
}

fn read_value(params: &str, act_id: i32) -> Option<i32> {
    let (actual, percent) = params.split_once('#')?;
    if actual.parse::<i32>().ok()? != act_id {
        return None;
    }
    let (whole, tenth) = percent.split_once('.')?;
    let [digit @ b'0'..=b'9'] = tenth.as_bytes() else {
        return None;
    };
    let whole = whole.parse::<i32>().ok().filter(|n| *n >= 0)?;
    whole.checked_mul(10)?.checked_add(i32::from(*digit - b'0'))
}

#[cfg(test)]
mod tests;
