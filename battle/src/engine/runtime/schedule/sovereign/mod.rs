use super::*;

pub(super) fn run_unlock(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let owners = pool
        .attacker_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(DrainResult::default());
    }
    drain::run_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::Kind(EventKind::SovereignUnlock),
        drain::ReactionLane::BuffActs,
        Some(&owners),
    )
}
