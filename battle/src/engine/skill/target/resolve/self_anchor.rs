use crate::engine::skill::{
    effect::SkillEffectCatalog,
    target::{TargetPool, request::SOURCE_TARGET_CODE},
};

use super::TargetResolver;

impl TargetResolver {
    /// A self-target skill cast without a selection anchors its action on the
    /// first unit of the caster's lineup; its effects still resolve on the
    /// caster.
    pub(crate) fn self_skill_anchor(
        catalog: &SkillEffectCatalog,
        pool: &TargetPool,
        source_uid: i64,
        skill_id: i32,
    ) -> Option<i64> {
        (catalog.logic_target(skill_id) == SOURCE_TARGET_CODE)
            .then(|| pool.main_allies(source_uid).first().map(|ally| ally.uid))
            .flatten()
    }
}
