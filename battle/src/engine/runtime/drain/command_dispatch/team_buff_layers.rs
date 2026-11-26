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
        ReactionAccess {
            pool: access.pool,
            managers: access.managers,
            catalog: access.catalog,
            determinism: access.determinism,
        },
        access.state,
        &[events],
        EventFrame {
            parent_path: settlement,
            reuse_path: path,
            action_path: scope.action_scope.map(Vec::as_slice),
            entry_halo_scope: Some(scope.entry_halo_scope),
            current_skill: scope.current_skill,
        },
        ReactionSelection {
            include_attack_consumption: false,
            execute_unscoped_after_action: scope.context.current_round > 0,
            publication_phase: crate::engine::event::subscription::PublicationPhase::AfterPublish,
            owner_uids,
            lane: None,
        },
        None,
        None,
    )
}
