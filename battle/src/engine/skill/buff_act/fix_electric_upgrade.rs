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
mod tests {
    use super::*;

    #[test]
    fn allied_modifier_replaces_the_configured_transition_destination() {
        let feature = ActiveBuffFeature {
            owner_uid: 20,
            source_uid: 20,
            buff_uid: 30,
            buff_id: 31280117,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "FixElectricUpgrade".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "1032#2#40#3".to_owned(),
            values: vec![1032, 2, 40, 3],
        };
        let thresholds = resolve_thresholds(
            1,
            &[
                FieldThreshold {
                    level: 2,
                    progress: 50,
                    definition: crate::engine::manager::field::FieldDefinition {
                        field_id: 30002,
                        duration: 3,
                    },
                },
                FieldThreshold {
                    level: 3,
                    progress: 120,
                    definition: crate::engine::manager::field::FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                },
            ],
            &[feature],
        );

        assert_eq!(
            thresholds,
            [
                FieldThreshold {
                    level: 3,
                    progress: 90,
                    definition: crate::engine::manager::field::FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                },
                FieldThreshold {
                    level: 3,
                    progress: 120,
                    definition: crate::engine::manager::field::FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                }
            ]
        );
    }
}
