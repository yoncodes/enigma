use super::*;

pub(in super::super) struct PreparedEvents {
    pub(in super::super) replacement: Option<BuffReplacementParts>,
    pub(in super::super) event_groups: Vec<Vec<BattleEvent>>,
    pub(in super::super) started_after_reactions: Option<ReactionBatch>,
    pub(in super::super) released_hit_events: Option<Vec<Vec<BattleEvent>>>,
    pub(in super::super) releases_pending_hits: bool,
    pub(in super::super) releases_hit_passive_skills: bool,
    pub(in super::super) releases_after_hit_skills: bool,
}

pub(in super::super) fn prepare(
    access: DrainAccess<'_>,
    scope: Scope<'_>,
    pending_hits: &mut HashMap<FramePath, Vec<Vec<BattleEvent>>>,
    outcome: &RuleOutcome,
    fanout: &[BuffFanoutResult],
    events: &[BattleEvent],
    mut event_groups: Vec<Vec<BattleEvent>>,
) -> Result<PreparedEvents, DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    let Scope {
        context,
        frame_path,
        event_scope,
        action_scope,
        entry_halo_scope,
        current_skill,
        ..
    } = scope;
    let action_scope = action_scope.map(Vec::as_slice);
    let replacement = match outcome {
        RuleOutcome::Buff(changes) => changes.replacement_parts(),
        _ => None,
    };
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Drain)
        && let RuleOutcome::Buff(changes) = outcome
        && changes.change.added.is_some()
        && !changes.change.removed.is_empty()
    {
        eprintln!(
            "drain buff replacement split={} removed={:?} added={:?}",
            replacement.is_some(),
            changes
                .change
                .removed
                .iter()
                .map(|removed| (removed.target_uid, removed.buff.buff_id, removed.buff.uid))
                .collect::<Vec<_>>(),
            changes.change.added.as_ref().map(|added| (
                added.target_uid,
                added.buff.buff_id,
                added.buff.uid
            )),
        );
    }
    let replaced_copy_uids = fanout
        .iter()
        .flat_map(|fanout| &fanout.allies)
        .flat_map(|ally| &ally.replaced)
        .filter_map(|removed| removed.buff.uid)
        .collect::<Vec<_>>();
    let is_replaced_copy = |event: &BattleEvent| {
        matches!(event, BattleEvent::BuffRemoved(change)
            if replaced_copy_uids.contains(&change.buff_uid))
    };
    let all_event_groups =
        (replacement.is_some() || !replaced_copy_uids.is_empty()).then(|| event_groups.clone());
    if let Some(parts) = &replacement {
        push_change(
            &mut result.frames,
            frame_path,
            crate::engine::runtime::change::BattleChange::Buff(Box::new(parts.lead.clone())),
        );
        let (removal_events, addition_events): (Vec<_>, Vec<_>) =
            events.iter().cloned().partition(|event| {
                matches!(event, BattleEvent::BuffRemoved(change)
                    if parts.removed_buff_uids.contains(&change.buff_uid))
            });
        let removal_reactions = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            &[removal_events],
            event_scope,
            frame_path,
            action_scope,
            entry_halo_scope,
            current_skill,
            false,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            None,
            None,
            None,
            None,
        )?;
        if !removal_reactions.before_publish.is_empty() {
            drain_nested_queue(
                managers,
                pool,
                catalog,
                determinism,
                removal_reactions.before_publish.into(),
                result,
                state,
            )?;
        }
        if !parts.removed_buff_uids.is_empty() {
            push_change(
                &mut result.frames,
                frame_path,
                crate::engine::runtime::change::BattleChange::Buff(Box::new(parts.removal.clone())),
            );
        }
        event_groups = vec![addition_events];
    }

    for group in &mut event_groups {
        group.retain(|event| !is_replaced_copy(event));
    }

    // BeforePublish reactions run before this outcome is recorded in
    // its semantic frame. Their own manager commits use nested drains.
    let mut before_reactions = dispatch_event_groups(
        pool,
        managers,
        catalog,
        determinism,
        state,
        &event_groups,
        event_scope,
        frame_path,
        action_scope,
        entry_halo_scope,
        current_skill,
        matches!(outcome, RuleOutcome::HpBatch(_)),
        context.current_round > 0,
        crate::engine::event::subscription::PublicationPhase::BeforePublish,
        None,
        None,
        None,
        None,
    )?;
    if let Some(groups) = all_event_groups {
        event_groups = groups;
    }
    let mut started_after_reactions = None;
    if matches!(
        outcome,
        RuleOutcome::SkillActionStarted { .. }
            | RuleOutcome::ActionCostPaid(_)
            | RuleOutcome::PaidSkillStarted(_)
    ) {
        let mut started_reactions = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            &event_groups,
            event_scope,
            frame_path,
            action_scope,
            entry_halo_scope,
            current_skill,
            matches!(outcome, RuleOutcome::HpBatch(_)),
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            None,
            None,
            None,
            None,
        )?;
        before_reactions
            .before_publish
            .append(&mut started_reactions.after_publish);
        before_reactions.assign_subscriber_death_groups(action_scope);
        started_after_reactions = Some(started_reactions);
    }
    result.events.extend(events.iter().cloned());
    let releases_pending_hits = matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if matches!(
            event.phase,
            crate::engine::skill::action::SkillPhase::AdditionalDamage
                | crate::engine::skill::action::SkillPhase::HitPassives
        )
    );
    let releases_hit_passive_skills = matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if event.phase == crate::engine::skill::action::SkillPhase::HitPassives
    );
    let releases_after_hit_skills = matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if event.phase == crate::engine::skill::action::SkillPhase::AfterHit
    );
    let mut pending_hit_skills = ReactionBatch::default();
    let released_hit_events = releases_pending_hits
        .then(|| pending_hits.remove(frame_path))
        .flatten();
    if let Some(hit_event_groups) = released_hit_events.as_ref() {
        let hit_events = flatten_event_groups(hit_event_groups);
        let hit_skills = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            hit_event_groups,
            event_scope,
            frame_path,
            action_scope,
            entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            None,
            Some(ReactionLane::Skills),
            None,
            None,
        )?;
        result.events.extend(hit_events.iter().cloned());
        pending_hit_skills = hit_skills;
    }
    if !before_reactions.before_publish.is_empty() {
        drain_nested_queue(
            managers,
            pool,
            catalog,
            determinism,
            before_reactions.before_publish.into(),
            result,
            state,
        )?;
    }
    if !pending_hit_skills.before_publish.is_empty()
        || !pending_hit_skills.after_publish.is_empty()
        || !pending_hit_skills.shared_hit_skills.is_empty()
    {
        let shared_hit_skills = std::mem::take(&mut pending_hit_skills.shared_hit_skills);
        state.defer_hit_skills(action_scope, shared_hit_skills);
        state.defer_hit_passive_skills(action_scope, pending_hit_skills.into_ordered());
    }

    Ok(PreparedEvents {
        replacement,
        event_groups,
        started_after_reactions,
        released_hit_events,
        releases_pending_hits,
        releases_hit_passive_skills,
        releases_after_hit_skills,
    })
}
