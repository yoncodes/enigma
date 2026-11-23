use crate::engine::skill::{
    behavior::classify::BehaviorKind, effect::SkillEffectCatalog,
    target::request::SOURCE_TARGET_CODE,
};

use super::ALLY_TARGET_LIMIT;

pub(super) fn uses_enemy_aim(catalog: &SkillEffectCatalog, skill_id: i32) -> bool {
    catalog.logic_target(skill_id) == SOURCE_TARGET_CODE
        && catalog.target_limit(skill_id) == ALLY_TARGET_LIMIT
        && catalog.is_active(skill_id)
        && !catalog.is_big_skill(skill_id)
        && catalog.get(skill_id).is_some_and(|effect| {
            effect.slots.iter().any(|slot| {
                slot.behavior.spec.kind == BehaviorKind::HeatScaleUseSkillAddCount
                    && slot.behavior.arg(0).is_some_and(|amount| amount > 0)
                    && slot.target.code == SOURCE_TARGET_CODE
            })
        })
}
