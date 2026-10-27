use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::classify::BehaviorKind,
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool, TargetRequest, TargetResolver},
    },
};

pub(crate) struct CardSelectionResolver;

impl CardSelectionResolver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn candidates(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        catalog: &SkillEffectCatalog,
        determinism: &RoundDeterminism,
        managers: Option<&BattleManagers>,
        context: TargetContext,
    ) -> Vec<i64> {
        if request.code == super::request::SOURCE_TARGET_CODE
            && catalog.get(skill_id).is_some_and(|effect| {
                effect
                    .slots
                    .iter()
                    .any(|slot| slot.behavior.spec.kind == BehaviorKind::AddConduitPower)
            })
        {
            return pool
                .enemies(source_uid, true)
                .iter()
                .filter(|target| target.current_hp > 0)
                .map(|target| target.uid)
                .collect();
        }

        TargetResolver::resolve_primary_candidates(
            request,
            skill_id,
            source_uid,
            pool,
            determinism,
            managers,
            context,
        )
    }
}

#[cfg(test)]
mod tests;
