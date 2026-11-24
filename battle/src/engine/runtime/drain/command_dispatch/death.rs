use super::*;

pub(super) struct Scope<'a> {
    pub(super) trigger: &'a SkillOpTrigger,
    pub(super) frame_path: &'a FramePath,
    pub(super) event_scope: &'a FramePath,
    pub(super) current_skill: Option<(i64, i32, Option<i64>)>,
    pub(super) subscriber: Option<&'a SubscriberContext>,
}

pub(super) struct Pending<'a> {
    pub(super) pending_deaths: Vec<PendingDeath>,
    pub(super) original_pending_deaths: Vec<PendingDeath>,
    pub(super) settles_death_immediately: bool,
    pub(super) settlement_action_scope: Option<&'a FramePath>,
    pub(super) active_action_scope: Option<&'a FramePath>,
    pub(super) releases_pending_hits: bool,
}

pub(super) fn settle(
    access: DrainAccess<'_>,
    scope: Scope<'_>,
    deaths: Pending<'_>,
    queue: &mut VecDeque<QueuedOp>,
    mut after_publish: Vec<QueuedOp>,
    after_settlement: Vec<QueuedOp>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    let Scope {
        trigger,
        frame_path,
        event_scope,
        current_skill,
        subscriber,
    } = scope;
    let Pending {
        mut pending_deaths,
        original_pending_deaths,
        settles_death_immediately,
        settlement_action_scope,
        active_action_scope,
        releases_pending_hits,
    } = deaths;
    // Death-sensitive reactions declared before settlement get one
    // chance to change HP before death transitions are finalized.
    if managers.terminal_outcome().is_none() && !pending_deaths.is_empty() {
        drain_death_settlement_queue(
            managers,
            pool,
            catalog,
            determinism,
            after_publish.into(),
            result,
            state,
        )?;
        after_publish = after_settlement;
    } else {
        after_publish.extend(after_settlement);
    }

    // A pending death becomes a frame change only if HP is still zero.
    // Active-action deaths wait for the shared HitPassives release.
    let completed_death_event_groups =
        state.take_completed_death_event_groups(&original_pending_deaths);
    pending_deaths.retain(|death| !completed_death_event_groups.contains(death));
    let event_grouped_deaths = completed_death_event_groups
        .into_iter()
        .filter(|death| managers.hp.is_current_death(*death))
        .collect::<Vec<_>>();
    let settled_deaths = pending_deaths
        .into_iter()
        .filter(|death| managers.hp.is_current_death(*death))
        .collect::<Vec<_>>();
    if settles_death_immediately
        && let Some(source_uid) = current_skill.map(|skill| skill.0)
        && settled_deaths
            .iter()
            .any(|death| death.transition().target_uid == source_uid)
        && let Some(execution) = queue
            .iter_mut()
            .find(|queued| {
                matches!(queued.trigger, SkillOpTrigger::Active)
                    && queued.frame_path.as_ref() == Some(frame_path)
                    && matches!(queued.op, RuleOp::Skill(_))
            })
            .and_then(|queued| queued.skill_execution.as_mut())
    {
        execution.record_immediate_source_death();
    }
    let mut terminal_death = settled_deaths
        .first()
        .or_else(|| event_grouped_deaths.first())
        .map(|death| death.transition());
    let mut cleanup_deaths = event_grouped_deaths
        .iter()
        .map(|death| death.transition())
        .collect::<Vec<_>>();
    let mut non_action_deaths = Vec::new();
    if settles_death_immediately && let Some(action_scope) = settlement_action_scope {
        for death in &settled_deaths {
            let cleanup =
                queued_defeated_entity_halo_cleanup(managers, death.transition(), action_scope);
            drain_defeated_entity_halo_cleanup(
                managers,
                pool,
                catalog,
                determinism,
                cleanup,
                result,
                state,
            )?;
            push_death(&mut result.frames, action_scope, managers, *death);
        }
        cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
    } else if let Some(action_scope) = active_action_scope {
        state.record_deaths(action_scope.clone(), settled_deaths.iter().copied());
    } else {
        cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
        if let Some(group) = subscriber.and_then(|subscriber| subscriber.group.as_ref()) {
            state.record_subscriber_deaths(group, event_scope, settled_deaths.iter().copied());
            terminal_death = None;
        } else {
            non_action_deaths.extend(settled_deaths.iter().copied());
        }
    }
    if releases_pending_hits && let Some(action_scope) = active_action_scope {
        let deaths = state
            .take_deaths(action_scope)
            .unwrap_or_default()
            .into_iter()
            .filter(|death| managers.hp.is_current_death(*death))
            .collect::<Vec<_>>();
        for death in &deaths {
            let cleanup =
                queued_defeated_entity_halo_cleanup(managers, death.transition(), action_scope);
            drain_defeated_entity_halo_cleanup(
                managers,
                pool,
                catalog,
                determinism,
                cleanup,
                result,
                state,
            )?;
            push_death(&mut result.frames, action_scope, managers, *death);
        }
        terminal_death = terminal_death.or_else(|| deaths.first().map(|death| death.transition()));
        cleanup_deaths.extend(deaths.into_iter().map(|death| death.transition()));
    }
    let death_count = settled_deaths.len() + event_grouped_deaths.len();
    if death_count != 0
        && matches!(trigger, SkillOpTrigger::Active)
        && let Some(queued) = queue.iter_mut().find(|queued| {
            matches!(queued.trigger, SkillOpTrigger::Active)
                && queued.frame_path.as_ref() == Some(frame_path)
                && matches!(queued.op, RuleOp::Skill(_))
        })
        && let Some(execution) = queued.skill_execution.as_mut()
    {
        execution.record_kills(death_count as i32);
    }
    let cleanup_parent_path = settlement_action_scope
        .map(Vec::as_slice)
        .unwrap_or(event_scope.as_slice());
    after_publish.extend(cleanup_deaths.into_iter().filter_map(|death| {
        queued_defeated_owner_card_cleanup(
            pool,
            managers,
            death,
            cleanup_parent_path,
            state.defender_card_cleanup_owner(),
        )
    }));
    let mut death_settlements = Vec::new();
    for death in non_action_deaths {
        if let Some(cleanup) =
            queued_defeated_entity_halo_cleanup(managers, death.transition(), event_scope)
        {
            death_settlements.push(cleanup);
        }
        death_settlements.push(queued_death_settlement(death, event_scope));
    }
    after_publish.extend(death_settlements);
    if let Some(death) = terminal_death {
        after_publish.push(queued_terminal_settlement(
            BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            }),
            event_scope,
        ));
    }

    Ok(after_publish)
}
