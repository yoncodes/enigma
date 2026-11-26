use super::*;

pub(super) fn reactions(
    access: DrainAccess<'_>,
    scope: publication::Scope<'_>,
    events: Vec<BattleEvent>,
    owner_uids: Option<&[i64]>,
) -> Result<ReactionBatch, DrainError> {
    let path = scope.frame_path;
    let settlement = match scope.trigger {
        SkillOpTrigger::Event(_) if scope.current_skill.is_some() && path.len() > 1 => {
            &path[..path.len() - 1]
        }
        _ => scope.event_scope.as_slice(),
    };
    access.result.events.extend(events.iter().cloned());
    dispatch_event_groups(
        access.pool,
        access.managers,
        access.catalog,
        access.determinism,
        access.state,
        &[events],
        settlement,
        path,
        scope.action_scope.map(Vec::as_slice),
        scope.entry_halo_scope,
        scope.current_skill,
        false,
        scope.context.current_round > 0,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        owner_uids,
        None,
        None,
        None,
    )
}
