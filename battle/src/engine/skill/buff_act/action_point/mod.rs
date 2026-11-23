use crate::engine::manager::buff::ActiveBuffFeature;

use super::{is_kind, registry::BuffActKind};

pub fn skill_uses_action_point(
    features: &[ActiveBuffFeature],
    owner_uid: i64,
    is_big_skill: bool,
) -> bool {
    !features.iter().any(|feature| {
        feature.owner_uid == owner_uid
            && (is_kind(feature, BuffActKind::SkillNoUseActPoint)
                || (is_big_skill && is_kind(feature, BuffActKind::BigSkillNoUseActPoint)))
    })
}

#[cfg(test)]
mod tests;
