use crate::engine::{
    manager::{buff::ActiveBuffFeature, field::FieldThreshold},
    skill::buff_act::{is_kind, registry::BuffActKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldThresholdModifier {
    pub replaced_level: i32,
    pub additional_progress: i32,
    pub destination_level: i32,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [replaced_level, additional_progress, destination_level]
            if *replaced_level > 0
                && *additional_progress != 0
                && *destination_level > *replaced_level
    )
}

pub fn modifier(feature: &ActiveBuffFeature) -> Option<FieldThresholdModifier> {
    if !feature.owner_alive || !is_kind(feature, BuffActKind::FixElectricUpgrade) {
        return None;
    }
    let [_, replaced_level, additional_progress, destination_level] = feature.values.as_slice()
    else {
        return None;
    };
    (*replaced_level > 0 && *additional_progress != 0 && *destination_level > *replaced_level)
        .then_some(FieldThresholdModifier {
            replaced_level: *replaced_level,
            additional_progress: *additional_progress,
            destination_level: *destination_level,
        })
}

pub fn resolve_thresholds(
    team: i32,
    base: &[FieldThreshold],
    features: &[ActiveBuffFeature],
) -> Vec<FieldThreshold> {
    let modifiers = features
        .iter()
        .filter(|feature| feature.team_type == team)
        .filter_map(modifier)
        .collect::<Vec<_>>();
    base.iter()
        .map(|threshold| {
            let modifier = modifiers
                .iter()
                .find(|modifier| modifier.replaced_level == threshold.level);
            let destination = modifier.and_then(|modifier| {
                base.iter()
                    .find(|candidate| candidate.level == modifier.destination_level)
                    .map(|candidate| (modifier, candidate))
            });
            match destination {
                Some((modifier, destination)) => FieldThreshold {
                    level: destination.level,
                    progress: threshold
                        .progress
                        .saturating_add(modifier.additional_progress)
                        .max(0),
                    definition: destination.definition,
                },
                None => *threshold,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
