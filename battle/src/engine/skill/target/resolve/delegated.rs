use crate::engine::skill::{
    action::SkillRequest,
    target::{TargetPool, request::SOURCE_TARGET_CODE},
};

use super::TargetResolver;

const OWN_SIDE: i32 = 2;

impl TargetResolver {
    pub(crate) fn delegated_anchor(pool: &TargetPool, cast: SkillRequest) -> Option<i64> {
        let (limit, logic) = pool.catalog().skill_targeting(cast.skill_id)?;
        if limit != OWN_SIDE || logic.trim().parse::<i32>().ok()? != SOURCE_TARGET_CODE {
            return None;
        }
        pool.main_allies(cast.source_uid)
            .iter()
            .find(|ally| ally.current_hp > 0)
            .map(|ally| ally.uid)
    }
}
