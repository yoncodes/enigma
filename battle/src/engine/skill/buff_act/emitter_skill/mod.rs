use crate::engine::{manager::buff::ActiveBuffFeature, mechanic::impromptu::ImpromptuDefinition};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill_id] if *skill_id > 0)
}

pub fn resolve(
    base: Option<ImpromptuDefinition>,
    features: &[ActiveBuffFeature],
    emitter_uid: i64,
) -> Option<ImpromptuDefinition> {
    let base = base?;
    let skill_id = features
        .iter()
        .filter(|feature| {
            feature.owner_uid == emitter_uid
                && super::is_kind(feature, BuffActKind::ChangeEmitterSkill)
        })
        .filter_map(|feature| feature.values.get(1).copied())
        .find(|skill_id| *skill_id > 0);
    Some(skill_id.map_or(base, |skill_id| base.with_skill(skill_id)))
}

#[cfg(test)]
mod tests;
