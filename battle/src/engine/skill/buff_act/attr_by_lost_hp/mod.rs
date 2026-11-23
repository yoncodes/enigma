use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::ActiveBuffFeature,
        hp::{HpManager, HpState},
    },
};

mod action;
pub(crate) use action::OutgoingEvaluation;

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, hp: &HpManager) -> i32 {
    Steps::parse(&feature.raw).map_or(0, |steps| steps.delta(attr_id, hp.get(feature.owner_uid)))
}

struct Steps<'a> {
    size: i32,
    cap: i32,
    attrs: &'a str,
    values: &'a str,
    absolute_mode: Option<&'a str>,
}

impl<'a> Steps<'a> {
    fn parse(raw: &'a str) -> Option<Self> {
        let parts = raw.split('#').collect::<Vec<_>>();
        let (size, attrs, values, cap, absolute_mode) = match parts.as_slice() {
            [_, size, attrs, values, cap] => (*size, *attrs, *values, *cap, None),
            [_, size, attr, value, cap, "1", mode @ ("0" | "1")] => {
                (*size, *attr, *value, *cap, Some(*mode))
            }
            _ => return None,
        };
        let size = size.parse().ok()?;
        let cap = cap.parse().ok()?;
        (size > 0 && cap > 0).then_some(Self {
            size,
            cap,
            attrs,
            values,
            absolute_mode,
        })
    }

    fn delta(&self, attr_id: AttrId, state: HpState) -> i32 {
        if state.max <= 0 {
            return 0;
        }
        let value = self
            .attrs
            .split(',')
            .zip(self.values.split(','))
            .find_map(|(raw_attr, raw_value)| {
                (raw_attr.parse().ok().and_then(AttrId::from_raw) == Some(attr_id))
                    .then(|| raw_value.parse::<i32>().ok())
                    .flatten()
            })
            .unwrap_or_default();
        let missing_hp = (state.max - state.current).max(0);
        let missing = if self.absolute_mode.is_some() {
            missing_hp
        } else {
            ((i64::from(missing_hp) * 1000) / i64::from(state.max)) as i32
        };
        value * (missing / self.size).min(self.cap)
    }
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
