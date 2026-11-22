use crate::engine::skill::{
    effect::{SkillEffectCatalog, catalog::SkillEffectTag},
    target::{TargetPool, request::SOURCE_TARGET_CODE},
};

const ALLY_TARGET_LIMIT: usize = 2;

pub(super) fn uses_enemy_aim(catalog: &SkillEffectCatalog, skill_id: i32) -> bool {
    catalog.logic_target(skill_id) == SOURCE_TARGET_CODE
        && catalog.target_limit(skill_id) == ALLY_TARGET_LIMIT
        && catalog.is_active(skill_id)
        && !catalog.is_big_skill(skill_id)
        && catalog.effect_tag(skill_id) == SkillEffectTag::Buff as i32
}

pub(super) fn selectable_enemies(
    pool: &TargetPool,
    source_uid: i64,
) -> impl Iterator<Item = &super::super::pool::TargetEntity> {
    pool.enemies(source_uid, true)
        .iter()
        .filter(|enemy| enemy.can_receive_enemy_aim(pool.catalog()))
}
