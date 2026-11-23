use crate::engine::skill::{
    effect::{SkillEffectCatalog, catalog::SkillEffectTag},
    target::request::SOURCE_TARGET_CODE,
};

use super::ALLY_TARGET_LIMIT;

pub(super) fn uses_enemy_aim(catalog: &SkillEffectCatalog, skill_id: i32) -> bool {
    catalog.logic_target(skill_id) == SOURCE_TARGET_CODE
        && catalog.target_limit(skill_id) == ALLY_TARGET_LIMIT
        && catalog.is_active(skill_id)
        && !catalog.is_big_skill(skill_id)
        && catalog.effect_tag(skill_id) == SkillEffectTag::Buff as i32
}
