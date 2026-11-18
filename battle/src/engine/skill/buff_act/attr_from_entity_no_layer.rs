use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [source_attr, target_attr, rate]
            if *source_attr == AttrId::Attack.id()
                && AttrId::from_raw(*target_attr).is_some()
                && *rate > 0
    )
}

pub fn snapshot(source_attack: i32, values: &[i32]) -> Option<(AttrId, i32)> {
    let [_, source_attr, target_attr, rate] = values else {
        return None;
    };
    supports(&[*source_attr, *target_attr, *rate]).then_some(())?;
    let target_attr = AttrId::from_raw(*target_attr)?;
    let value = (i64::from(source_attack.max(0)) * i64::from(*rate) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    Some((target_attr, value))
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    if !super::is_kind(feature, super::registry::BuffActKind::AttrFromEntityNoLayer) {
        return 0;
    }
    let [act_id, _, configured_attr, _] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*configured_attr) != Some(attr_id) {
        return 0;
    }
    buffs
        .snapshot(feature.owner_uid, feature.buff_uid)
        .and_then(|buff| {
            buff.act_info
                .into_iter()
                .find(|info| info.act_id == Some(*act_id))
        })
        .and_then(|info| info.str_param)
        .and_then(|raw| {
            let (attr, value) = raw.split_once('#')?;
            (attr.parse::<i32>().ok()? == *configured_attr)
                .then(|| value.parse::<i32>().ok())
                .flatten()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
