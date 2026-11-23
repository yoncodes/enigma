use crate::engine::{
    manager::{BattleManagers, emanation::EmanationKind},
    skill::{
        behavior::crystal_card, effect::SkillEffectCatalog, target::request::SOURCE_TARGET_CODE,
    },
};

use super::ALLY_TARGET_LIMIT;

pub(super) fn uses_enemy_aim(
    catalog: &SkillEffectCatalog,
    skill_id: i32,
    source_uid: i64,
    managers: &BattleManagers,
) -> bool {
    catalog.logic_target(skill_id) == SOURCE_TARGET_CODE
        && catalog.target_limit(skill_id) == ALLY_TARGET_LIMIT
        && catalog.is_active(skill_id)
        && !catalog.is_big_skill(skill_id)
        && crystal_card::has_kind(
            managers,
            catalog,
            source_uid,
            skill_id,
            EmanationKind::Purple,
        )
}
