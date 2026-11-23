use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [tracked_buff_id, 1, configured_attr, per_layer]
            if *tracked_buff_id > 0
                && matches!(
                    AttrId::from_raw(*configured_attr),
                    Some(AttrId::DmgBonus | AttrId::GenesisDmgBonus)
                )
                && *per_layer > 0
    )
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    if !supports(feature.values.get(1..).unwrap_or_default()) {
        return 0;
    }
    let [_, tracked_buff_id, 1, configured_attr, per_layer, ..] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*configured_attr) != Some(attr_id) {
        return 0;
    }

    let tracker_uid = if feature.source_uid != 0 {
        feature.source_uid
    } else {
        feature.owner_uid
    };
    buffs
        .grant_value(feature.buff_uid, feature.values[0])
        .unwrap_or_else(|| buffs.buff_id_or_type_amount(tracker_uid, *tracked_buff_id))
        * per_layer
}

#[cfg(test)]
mod tests;
