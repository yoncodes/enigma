use crate::engine::{
    entity::attr::AttrId,
    manager::{buff::ActiveBuffFeature, hp::HpManager},
};

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, hp: &HpManager) -> i32 {
    let parts = feature.raw.split('#').collect::<Vec<_>>();
    let Some((step, raw_attrs, raw_values, max_steps, absolute_step)) = (match parts.as_slice() {
        [_, step, attrs, values, max_steps] => Some((*step, *attrs, *values, *max_steps, false)),
        [_, step, attr, value, max_steps, "1", mode] if matches!(*mode, "0" | "1") => {
            Some((*step, *attr, *value, *max_steps, true))
        }
        _ => None,
    }) else {
        return 0;
    };
    let Ok(step) = step.parse::<i32>() else {
        return 0;
    };
    let Ok(max_steps) = max_steps.parse::<i32>() else {
        return 0;
    };
    if step <= 0 || max_steps <= 0 {
        return 0;
    }
    let Some(value) =
        raw_attrs
            .split(',')
            .zip(raw_values.split(','))
            .find_map(|(raw_attr, raw_value)| {
                (raw_attr.parse().ok().and_then(AttrId::from_raw) == Some(attr_id))
                    .then(|| raw_value.parse::<i32>().ok())
                    .flatten()
            })
    else {
        return 0;
    };
    let state = hp.get(feature.owner_uid);
    if state.max <= 0 {
        return 0;
    }
    let missing_hp = (state.max - state.current).max(0);
    let missing = if absolute_step {
        missing_hp
    } else {
        ((i64::from(missing_hp) * 1000) / i64::from(state.max)) as i32
    };
    value * (missing / step).min(max_steps)
}

pub fn supports(args: &[i32]) -> bool {
    if let [step, attr, _, max_steps, 1, mode] = args {
        return *step > 0
            && AttrId::from_raw(*attr).is_some()
            && *max_steps > 0
            && matches!(*mode, 0 | 1);
    }
    matches!(args, [step, first_attr, second_attr, .., max_steps]
        if *step > 0
            && AttrId::from_raw(*first_attr).is_some()
            && AttrId::from_raw(*second_attr).is_some()
            && *max_steps > 0)
}

#[cfg(test)]
mod tests;
