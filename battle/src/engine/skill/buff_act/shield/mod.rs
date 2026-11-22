use crate::engine::{entity::attr::AttrId, manager::buff::BuffManager, skill::buff_act::registry};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [_, raw_attr, rate, ..]
        if AttrId::from_raw(*raw_attr).is_some() && *rate > 0)
}

pub fn configured_attr_rate(
    buff_id: i32,
    source_uid: i64,
    buffs: &BuffManager,
) -> Option<(AttrId, i32)> {
    let catalog = buffs
        .try_catalog()
        .or_else(crate::catalog::BattleCatalog::try_global)?;
    let features = catalog.buff_feature_rows(buff_id);
    features.iter().find_map(|feature| {
        let values = feature
            .split('#')
            .map(str::trim)
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let act_id = *values.first()?;
        let act_type = catalog.buff_act_definition(act_id)?.key.type_name;
        match registry::kind(act_id, act_type)? {
            registry::BuffActKind::Shield => {
                let [_, _, raw_attr, rate, ..] = values.as_slice() else {
                    return None;
                };
                Some((AttrId::from_raw(*raw_attr)?, *rate))
            }
            registry::BuffActKind::ShieldByBuffLayer => {
                let [
                    _,
                    raw_attr,
                    base_rate,
                    tracked_buff_id,
                    per_layer,
                    max_layers,
                ] = values.as_slice()
                else {
                    return None;
                };
                let layers = buffs.buff_id_or_type_amount(source_uid, *tracked_buff_id);
                let layers = if *max_layers >= 0 {
                    layers.min(*max_layers)
                } else {
                    layers
                };
                let multiplier = 1000_i32.saturating_add(per_layer.saturating_mul(layers));
                Some((
                    AttrId::from_raw(*raw_attr)?,
                    base_rate.saturating_mul(multiplier) / 1000,
                ))
            }
            _ => None,
        }
    })
}

pub fn cumulative_attr_rate(buff_id: i32, buffs: &BuffManager) -> Option<(AttrId, i32)> {
    let catalog = buffs
        .try_catalog()
        .or_else(crate::catalog::BattleCatalog::try_global)?;
    catalog
        .buff_feature_rows(buff_id)
        .iter()
        .find_map(|feature| {
            let values = feature
                .split('#')
                .map(str::trim)
                .map(str::parse::<i32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            let [act_id, mode, raw_attr, rate, ..] = values.as_slice() else {
                return None;
            };
            let definition = catalog.buff_act_definition(*act_id)?;
            if registry::kind(*act_id, definition.key.type_name)? != registry::BuffActKind::Shield
                || *mode != 1
            {
                return None;
            }
            Some((AttrId::from_raw(*raw_attr)?, *rate))
        })
}

pub fn supports_by_buff_layer(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_attr, base_rate, tracked_buff_id, per_layer, max_layers]
            if AttrId::from_raw(*raw_attr).is_some()
                && *base_rate > 0
                && *tracked_buff_id > 0
                && *per_layer > 0
                && (*max_layers == -1 || *max_layers > 0)
    )
}

#[cfg(test)]
mod tests;
