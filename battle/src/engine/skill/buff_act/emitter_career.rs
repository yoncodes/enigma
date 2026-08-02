use crate::engine::manager::buff::ActiveBuffFeature;

use super::{is_kind, registry::BuffActKind};

pub fn career<'a>(features: impl IntoIterator<Item = &'a ActiveBuffFeature>) -> Option<i32> {
    features
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == crate::engine::manager::emitter::UID
                && is_kind(feature, BuffActKind::EmitterCareerChange)
        })
        .find_map(|feature| feature.values.get(1).copied())
        .filter(|career| *career > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_career_comes_from_its_own_opcode_876_feature() {
        assert_eq!(
            super::super::registry::destination(876, "EmitterCareerChange", &[2]),
            Some(super::super::registry::BuffActDestination::StateConsumer)
        );
        assert_eq!(
            super::super::registry::destination(876, "EmitterCareerChange", &[]),
            None
        );
        assert_eq!(
            super::super::registry::destination(876, "EmitterCareerChange", &[2, 3]),
            None
        );
        let feature = ActiveBuffFeature {
            owner_uid: crate::engine::manager::emitter::UID,
            source_uid: 1,
            buff_uid: 1,
            buff_id: 31080147,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "EmitterCareerChange".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "876#2".to_owned(),
            values: vec![876, 2],
        };

        assert_eq!(career([&feature]), Some(2));
    }
}
