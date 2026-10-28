use crate::engine::manager::buff::ActiveBuffFeature;

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [delta] if *delta > 0)
}

pub fn resolve(base: i32, features: &[ActiveBuffFeature], emitter_uid: i64) -> i32 {
    features
        .iter()
        .filter(|feature| {
            feature.owner_uid == emitter_uid
                && super::is_kind(feature, BuffActKind::ChangeEmitterCardEnergyLimit)
        })
        .find_map(|feature| feature.values.get(1).copied())
        .filter(|delta| *delta > 0)
        .map_or(base, |delta| base.saturating_add(delta))
}

#[cfg(test)]
mod tests;
