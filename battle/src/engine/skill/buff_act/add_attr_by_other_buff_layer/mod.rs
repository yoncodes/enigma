use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

#[derive(Clone, Copy)]
pub enum LayerScope {
    Source,
    SourceOrOwner,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [primary_attr, base, other_buff_id, per_layer, layer_limit, capped_attr, _]
            if AttrId::from_raw(*primary_attr).is_some()
                && *base != 0
                && *other_buff_id > 0
                && *per_layer != 0
                && *layer_limit > 0
                && AttrId::from_raw(*capped_attr).is_some()
    )
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &BuffManager,
    scope: LayerScope,
) -> i32 {
    let [
        _,
        primary_attr,
        base,
        other_buff_id,
        per_layer,
        layer_limit,
        rest @ ..,
    ] = feature.values.as_slice()
    else {
        return 0;
    };
    let source_layer = buffs.id_or_type_layer(feature.source_uid, *other_buff_id);
    let layer = match scope {
        LayerScope::Source => source_layer,
        LayerScope::SourceOrOwner => {
            source_layer.max(buffs.id_or_type_layer(feature.owner_uid, *other_buff_id))
        }
    }
    .min((*layer_limit).max(0));
    let primary = (*primary_attr == attr_id as i32).then_some(base + per_layer * layer);
    let capped = match rest {
        [capped_attr, capped_value, ..]
            if layer >= *layer_limit && *capped_attr == attr_id as i32 =>
        {
            Some(*capped_value)
        }
        _ => None,
    };
    primary.unwrap_or_default() + capped.unwrap_or_default()
}

#[cfg(test)]
mod tests;
