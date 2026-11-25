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

mod record_ultimate;
mod source_buff;
mod sovereign_charge;
mod sovereign_vision;

const ALLY_TARGET_LIMIT: usize = 2;

fn uses_enemy_aim(
    catalog: &SkillEffectCatalog,
    skill_id: i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    source_buff::uses_enemy_aim(catalog, skill_id)
        || record_ultimate::uses_enemy_aim(catalog, skill_id)
        || sovereign_charge::uses_enemy_aim(catalog, skill_id)
        || managers.is_some_and(|managers| {
            sovereign_vision::uses_enemy_aim(catalog, skill_id, source_uid, managers)
        })
}

fn selectable_enemies(
    pool: &TargetPool,
    source_uid: i64,
) -> impl Iterator<Item = &super::pool::TargetEntity> {
    pool.enemies(source_uid, true)
        .iter()
        .filter(|enemy| enemy.can_receive_enemy_aim(pool.catalog()))
}

impl CardSelectionResolver {
    pub(crate) fn source_aim(
        catalog: &SkillEffectCatalog,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        preferred_target: Option<i64>,
        managers: &BattleManagers,
    ) -> Option<i64> {
        if !uses_enemy_aim(catalog, skill_id, source_uid, Some(managers)) {
            return None;
        }
        Self::enemy_aim(source_uid, pool, preferred_target)
    }

    pub(crate) fn enemy_aim(
        source_uid: i64,
        pool: &TargetPool,
        preferred_target: Option<i64>,
    ) -> Option<i64> {
        selectable_enemies(pool, source_uid)
            .min_by_key(|enemy| {
                (
                    preferred_target != Some(enemy.uid),
                    enemy.position,
                    enemy.uid,
                )
            })
            .map(|enemy| enemy.uid)
    }

    pub(crate) fn is_enemy_aim(source_uid: i64, target_uid: i64, pool: &TargetPool) -> bool {
        selectable_enemies(pool, source_uid).any(|enemy| enemy.uid == target_uid)
    }

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
                effect.slots.iter().any(|slot| {
                    matches!(
                        slot.behavior.spec.kind,
                        BehaviorKind::AddConduitPower | BehaviorKind::HedoneAddDevicePower
                    )
                })
            })
        {
            return pool
                .enemies(source_uid, true)
                .iter()
                .filter(|target| target.current_hp > 0)
                .map(|target| target.uid)
                .collect();
        }

        let mut candidates = TargetResolver::resolve_primary_candidates(
            request,
            skill_id,
            source_uid,
            pool,
            determinism,
            managers,
            context,
        );
        if uses_enemy_aim(catalog, skill_id, source_uid, managers) {
            candidates.extend(selectable_enemies(pool, source_uid).map(|enemy| enemy.uid));
        }
        candidates
    }
}

#[cfg(test)]
mod tests;
