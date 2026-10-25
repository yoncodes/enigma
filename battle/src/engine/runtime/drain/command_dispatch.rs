use super::{command_prepare::prepare_command, *};

#[allow(clippy::too_many_arguments)]
pub(super) fn drain_command(
    managers: &mut BattleManagers,
    base_pool: &TargetPool,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
    result: &mut DrainResult,
    state: &mut DrainState,
    bus: &mut EventBus,
    pending_hits: &mut HashMap<FramePath, Vec<Vec<BattleEvent>>>,
    mut command: RuleOp,
    trigger: SkillOpTrigger,
    skill_execution: Option<SkillExecution>,
    frame_path: Option<FramePath>,
    parent_path: Option<FramePath>,
    frame_group: Option<Rc<RefCell<Option<FramePath>>>>,
    independent_parent_group: Option<Rc<RefCell<Option<FramePath>>>>,
    frame_owner: Option<FrameOwner>,
    subscriber: Option<SubscriberContext>,
    caster_frame: Option<FrameOwner>,
) -> Result<(), DrainError> {
    if subscriber
        .as_ref()
        .is_some_and(|subscriber| subscriber.intermediate_death_group)
        && let RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(command)) =
            &mut command
        && let crate::engine::manager::buff::BuffCommand::GrantOnDeath(grant) = command
    {
        *command = crate::engine::manager::buff::BuffCommand::GrantOnIntermediateDeath(*grant);
    }
    // After-hit observers must execute before the lifecycle operation
    // that closes their boundary, so requeue that closer behind them.
    let releases_after_hit = matches!(
        &command,
        RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if event.phase == crate::engine::skill::action::SkillPhase::AfterHit
    ) || matches!(
        &command,
        RuleOp::SkillLifecycle(crate::engine::skill::action::SkillLifecycle::ActionCompleted(_))
    );
    let action_scope = frame_path
        .as_deref()
        .and_then(|path| active_skill_scope_path(&result.frames, path));
    if releases_after_hit && let Some(observers) = state.take_after_hit(action_scope.as_ref()) {
        queue.push_front(QueuedOp {
            op: command,
            trigger,
            skill_execution,
            frame_path,
            parent_path,
            frame_group,
            independent_parent_group,
            frame_owner,
            subscriber,
            caster_frame,
        });
        prepend(queue, observers);
        return Ok(());
    }
    let completes_action = matches!(
        &command,
        RuleOp::SkillLifecycle(crate::engine::skill::action::SkillLifecycle::ActionCompleted(_))
    );
    let deferred_followup_owner = matches!(
        &command,
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::ThresholdSkill(_))
    )
    .then(|| frame_owner.clone())
    .flatten();
    let frame_path = if deferred_followup_owner.is_some() {
        frame_path.or(parent_path).unwrap_or_else(|| {
            push_root(
                &mut result.frames,
                FrameOwner::Command,
                frame_trigger(&trigger),
            )
        })
    } else {
        ensure_frame(
            &mut result.frames,
            frame_path,
            parent_path.as_deref(),
            frame_owner.unwrap_or(FrameOwner::Command),
            &trigger,
        )
    };
    if deferred_followup_owner.is_none()
        && let Some(group) = &frame_group
    {
        *group.borrow_mut() = Some(frame_path.clone());
    }
    if let RuleOp::SkillLifecycle(crate::engine::skill::action::SkillLifecycle::PhaseCompleted(
        event,
    )) = &mut command
        && let Some(execution) = queue.iter().find_map(|queued| {
            (matches!(queued.trigger, SkillOpTrigger::Active)
                && queued.frame_path.as_ref() == Some(&frame_path))
            .then_some(queued.skill_execution.as_ref())
            .flatten()
        })
    {
        execution.sync_lifecycle_event(event);
    }
    if let RuleOp::SkillLifecycle(lifecycle) = &mut command
        && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
        && let Some(injuries) = state.injuries(&action_path)
    {
        let source_uid = match lifecycle {
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event) => event.source_uid,
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(event) => {
                event.source_uid
            }
            _ => 0,
        };
        let source_is_attacker = pool.source_is_attacker(source_uid);
        let count = injuries
            .iter()
            .filter(|target_uid| pool.source_is_attacker(**target_uid) == source_is_attacker)
            .count() as i32;
        match lifecycle {
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event) => {
                event.teammate_injury_count = count;
                event.teammate_injury_count_not_reset = count;
            }
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(event) => {
                event.teammate_injury_count = count;
                event.teammate_injury_count_not_reset = count;
            }
            _ => {}
        }
    }
    command = prepare_command(managers, pool, determinism, command);

    let entry_halo_fanout = matches!(
        &command,
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
            crate::engine::manager::buff::BuffCommand::FanoutEntryHalos(_)
        ))
    );

    // This is the single durable commit point for non-skill RuleOps.
    // The returned outcome describes committed changes and follow-ups.
    let mut outcome = execute_rule_op_with(managers, bus, determinism, command)?;
    let transformed = match &outcome {
        RuleOutcome::Entity(changes)
            if matches!(
                changes.operation,
                crate::engine::manager::entity::EntityOperation::Transform { .. }
            ) =>
        {
            Some(changes.as_ref())
        }
        RuleOutcome::MonsterChange(changes) => Some(&changes.entity),
        _ => None,
    };
    if let Some(changes) = transformed {
        let current_pool = base_pool.runtime_view(managers);
        let active_skills =
            crate::engine::skill::subscriber::active_skills(&current_pool, managers);
        state.retire_replaced_subscriber_casts(
            changes.target_uid,
            active_skills.iter().filter_map(|(owner_uid, skill_id)| {
                (*owner_uid == changes.target_uid).then_some(*skill_id)
            }),
        );
    }
    if let RuleOutcome::ActiveSkillTargetsModified(additional_count) = outcome {
        let action_scope = action_scope
            .clone()
            .ok_or(DrainError::MissingActiveSkillContext)?;
        state.add_target_modifier(action_scope, additional_count);
    }
    let followups = outcome.followups();
    let damage_amount = outcome.applied_damage();
    let death_count = outcome.death_count();
    let guard_break_count = outcome.guard_break_count();
    let injured_targets = outcome.injured_targets();
    if !injured_targets.is_empty()
        && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
    {
        state.record_injuries(action_path, &injured_targets);
    }
    if (damage_amount > 0
        || death_count > 0
        || guard_break_count > 0
        || !injured_targets.is_empty())
        && matches!(trigger, SkillOpTrigger::Active)
        && let Some(queued) = queue.iter_mut().find(|queued| {
            matches!(queued.trigger, SkillOpTrigger::Active)
                && queued.frame_path.as_ref() == Some(&frame_path)
                && matches!(queued.op, RuleOp::Skill(_))
        })
    {
        let source_uid = match &queued.op {
            RuleOp::Skill(invocation) => invocation.plan.source_uid,
            _ => 0,
        };
        let source_is_attacker = pool.source_is_attacker(source_uid);
        let allied_injuries = injured_targets
            .into_iter()
            .filter(|target_uid| pool.source_is_attacker(*target_uid) == source_is_attacker);
        if let Some(execution) = queued.skill_execution.as_mut() {
            execution.record_damage(damage_amount);
            execution.record_guard_breaks(guard_break_count);
            execution.record_injuries(allied_injuries);
        }
    }
    let event_scope = event_scope_path(&result.frames, &frame_path);
    let entry_halo_scope = entity_entry_halo_scope_path(&result.frames, &frame_path);
    let fanout = match &outcome {
        RuleOutcome::Buff(changes) => changes.fanout.clone(),
        _ => Vec::new(),
    };
    let current_skill = match owner_at_path(&result.frames, &frame_path) {
        FrameOwner::Skill {
            source_uid,
            skill_id,
            target_uid,
            ..
        }
        | FrameOwner::ConduitSkill {
            source_uid,
            skill_id,
            target_uid,
            ..
        } => Some((*source_uid, *skill_id, *target_uid)),
        _ => None,
    };
    let fanout_output_owner = if entry_halo_fanout || current_skill.is_none() {
        crate::engine::skill::behavior::registry::OutputOwner::Skill
    } else {
        crate::engine::skill::behavior::registry::OutputOwner::CausingEvent.resolve(
            match trigger {
                SkillOpTrigger::Event(ref event) => Some(event.kind()),
                _ => None,
            },
            false,
        )
    };

    // Managers publish semantic events through the bus. Reactions are
    // derived from these committed events, never fired by packet code.
    let mut events = std::iter::from_fn(|| bus.pop()).collect::<Vec<_>>();
    let mut event_groups = match &outcome {
        RuleOutcome::HpBatch(batch) => grouped_hp_events(
            &events,
            batch
                .iter()
                .map(|execution| execution.changes.events())
                .collect(),
        ),
        _ => vec![events.clone()],
    };
    if matches!(trigger, SkillOpTrigger::Active)
        && let Some(queued) = queue.iter_mut().find(|queued| {
            matches!(queued.trigger, SkillOpTrigger::Active)
                && queued.frame_path.as_ref() == Some(&frame_path)
                && matches!(queued.op, RuleOp::Skill(_))
        })
        && let Some(execution) = queued.skill_execution.as_mut()
    {
        execution.record_attacks(events.iter().filter_map(|event| {
            let BattleEvent::Hit(hit) = event else {
                return None;
            };
            matches!(
                hit.damage_from,
                crate::engine::manager::hp::HurtDamageFromType::Skill
                    | crate::engine::manager::hp::HurtDamageFromType::SkillEffect
            )
            .then_some(hit.target_uid)
        }));
        let source_uid = current_skill.map(|skill| skill.0).unwrap_or_default();
        execution.record_buff_additions(events.iter().filter_map(|event| {
            let (BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change)) = event else {
                return None;
            };
            (change.source_uid == source_uid).then_some((
                change.buff_id,
                change.after_amount.saturating_sub(change.before_amount),
            ))
        }));
    }

    let action_scope = active_skill_scope_path(&result.frames, &frame_path);
    let has_active_continuation = queue.iter().any(|queued| {
        matches!(&queued.trigger, SkillOpTrigger::Active)
            && queued.frame_path.as_ref() == Some(&frame_path)
            && matches!(&queued.op, RuleOp::Skill(_))
    });
    let defer_hits = matches!(&outcome, RuleOutcome::HpBatch(_))
        && matches!(trigger, SkillOpTrigger::Active)
        && current_skill.is_some()
        && has_active_continuation;
    if defer_hits {
        // Multi-part active hits share one HitPassives boundary. Hold
        // primary hits and their own HP loss. Their BeforePublish
        // buff acts still belong before this HP outcome.
        let (immediate, deferred) = split_active_hit_event_groups(event_groups);
        let hit_buff_acts = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            &deferred,
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            None,
            Some(ReactionLane::BuffActs),
            None,
        )?;
        drain_nested_queue(
            managers,
            pool,
            catalog,
            determinism,
            hit_buff_acts.into_ordered().into(),
            result,
            state,
        )?;
        pending_hits
            .entry(frame_path.clone())
            .or_default()
            .extend(deferred);
        event_groups = immediate;
        events = flatten_event_groups(&event_groups);
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
        &event_scope,
        &frame_path,
        action_scope.as_deref(),
        &entry_halo_scope,
        current_skill,
        matches!(&outcome, RuleOutcome::HpBatch(_)),
        context.current_round > 0,
        crate::engine::event::subscription::PublicationPhase::BeforePublish,
        None,
        None,
        None,
    )?;
    let mut started_after_reactions = None;
    if matches!(&outcome, RuleOutcome::SkillActionStarted { .. }) {
        let mut started_reactions = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            &event_groups,
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            matches!(&outcome, RuleOutcome::HpBatch(_)),
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            None,
            None,
            None,
        )?;
        before_reactions
            .before_publish
            .append(&mut started_reactions.after_publish);
        before_reactions.assign_subscriber_death_groups(action_scope.as_deref());
        started_after_reactions = Some(started_reactions);
    }
    result.events.extend(events.iter().cloned());
    let releases_pending_hits = matches!(
        &outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if matches!(
            event.phase,
            crate::engine::skill::action::SkillPhase::AdditionalDamage
                | crate::engine::skill::action::SkillPhase::HitPassives
        )
    );
    let releases_hit_passive_skills = matches!(
        &outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if event.phase == crate::engine::skill::action::SkillPhase::HitPassives
    );
    let releases_after_hit_skills = matches!(
        &outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
        ) if event.phase == crate::engine::skill::action::SkillPhase::AfterHit
    );
    let mut pending_hit_skills = ReactionBatch::default();
    let released_hit_events = releases_pending_hits
        .then(|| pending_hits.remove(&frame_path))
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
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            None,
            Some(ReactionLane::Skills),
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
        state.defer_hit_skills(action_scope.as_deref(), shared_hit_skills);
        state.defer_hit_passive_skills(action_scope.as_deref(), pending_hit_skills.into_ordered());
    }

    // Once pre-publication work is complete, record the authoritative
    // outcome and any buff fanout beneath their semantic owners.
    let settles_death_immediately = outcome.settles_death_immediately();
    let mut pending_deaths = outcome.take_deaths();
    let settlement_action_scope = action_scope
        .as_ref()
        .filter(|path| matches!(trigger, SkillOpTrigger::Active) || state.action_in_progress(path));
    let active_action_scope = (!settles_death_immediately)
        .then_some(settlement_action_scope)
        .flatten();
    let original_pending_deaths = pending_deaths.clone();
    let redeal_hand = match &outcome {
        RuleOutcome::Card(changes)
            if changes.kind == crate::engine::manager::card::CardChangeKind::RedealtKeepRanks =>
        {
            Some(changes.after.clone())
        }
        _ => None,
    };
    let changes = outcome.changes();
    for change in changes {
        push_change(&mut result.frames, &frame_path, change);
    }
    if let Some(cards) = redeal_hand {
        push_cue(&mut result.frames, RoundCue::RedealHandSync { cards });
    }
    for (owner, change) in outcome.owned_changes() {
        if owner == crate::engine::skill::behavior::registry::OutputOwner::NestedLifecycleMechanic {
            let nested = push_child(
                &mut result.frames,
                &frame_path,
                FrameOwner::LifecycleMechanic,
                frame_trigger(&trigger),
            );
            push_change(&mut result.frames, &nested, change);
        } else {
            push_change(
                &mut result.frames,
                &output_frame_path(owner, &frame_path),
                change,
            );
        }
    }
    for fanout in fanout {
        let fanout_parent = output_frame_path(fanout_output_owner, &event_scope);
        let fanout_path = push_child(
            &mut result.frames,
            &fanout_parent,
            FrameOwner::BuffRule {
                emitter_uid: fanout.emitter_uid,
                carrier_buff_uid: fanout.carrier_buff_uid,
                carrier_buff_id: fanout.carrier_buff_id,
                rule: fanout.rule,
            },
            frame_trigger(&trigger),
        );
        push_change(
            &mut result.frames,
            &fanout_path,
            crate::engine::runtime::change::BattleChange::BuffFanout(Box::new(fanout)),
        );
    }
    let was_skill_action_started = matches!(&outcome, RuleOutcome::SkillActionStarted { .. });
    let was_hp_batch = matches!(&outcome, RuleOutcome::HpBatch(_));
    result.outcomes.push(outcome);

    let terminal_owner_uids = managers
        .terminal_outcome()
        .and_then(|outcome| outcome.winning_team())
        .map(|winning_team| base_pool.team_uids(winning_team));

    // AfterPublish reactions are partitioned by their declared release
    // lane; after-hit and after-action work stays action-scoped in state.
    let mut reactions = if was_skill_action_started {
        started_after_reactions.unwrap_or_default()
    } else {
        dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            &event_groups,
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            was_hp_batch,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids.as_deref(),
            None,
            active_action_scope
                .is_some()
                .then_some(pending_deaths.as_slice()),
        )?
    };
    state.defer_hp_thresholds(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.hp_thresholds),
    );
    state.defer_attacked_skills(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.attacked_skills),
    );
    state.defer_before_after_hit_observers(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.before_after_hit_observers),
    );
    state.defer_after_hit_skills(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.after_hit_skills),
    );
    state.defer_after_hit_shell(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.after_hit_shell),
    );
    state.defer_after_hit(
        action_scope.as_deref(),
        std::mem::take(&mut reactions.after_hit),
    );
    let after_action = std::mem::take(&mut reactions.after_action);
    if action_scope.is_some() {
        state.defer_after_action(action_scope.as_deref(), after_action);
    } else {
        reactions.after_publish.extend(after_action);
    }
    if let Some(hit_event_groups) = released_hit_events.as_ref() {
        let mut hit_buff_acts = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            hit_event_groups,
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids.as_deref(),
            Some(ReactionLane::BuffActs),
            None,
        )?;
        let mut hit_skills = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            hit_event_groups,
            &event_scope,
            &frame_path,
            action_scope.as_deref(),
            &entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids.as_deref(),
            Some(ReactionLane::Skills),
            None,
        )?;
        hit_buff_acts
            .attacked_skills
            .append(&mut hit_skills.attacked_skills);
        state.defer_attacked_skills(
            action_scope.as_deref(),
            std::mem::take(&mut hit_buff_acts.attacked_skills),
        );
        hit_buff_acts
            .hp_thresholds
            .append(&mut hit_skills.hp_thresholds);
        state.defer_hp_thresholds(
            action_scope.as_deref(),
            std::mem::take(&mut hit_buff_acts.hp_thresholds),
        );
        hit_buff_acts
            .before_after_hit_observers
            .append(&mut hit_skills.before_after_hit_observers);
        state.defer_before_after_hit_observers(
            action_scope.as_deref(),
            std::mem::take(&mut hit_buff_acts.before_after_hit_observers),
        );
        hit_buff_acts
            .after_hit_skills
            .append(&mut hit_skills.after_hit_skills);
        state.defer_after_hit_skills(
            action_scope.as_deref(),
            std::mem::take(&mut hit_buff_acts.after_hit_skills),
        );
        hit_buff_acts
            .after_hit_shell
            .append(&mut hit_skills.after_hit_shell);
        state.defer_after_hit_shell(
            action_scope.as_deref(),
            std::mem::take(&mut hit_buff_acts.after_hit_shell),
        );
        state.defer_after_hit(
            action_scope.as_deref(),
            std::mem::take(&mut hit_skills.after_hit),
        );
        let hit_after_action = std::mem::take(&mut hit_buff_acts.after_action);
        if action_scope.is_some() {
            state.defer_after_action(action_scope.as_deref(), hit_after_action);
        } else {
            hit_buff_acts.after_publish.extend(hit_after_action);
        }
        let shared_hit_skills = std::mem::take(&mut hit_skills.shared_hit_skills);
        state.defer_hit_skills(action_scope.as_deref(), shared_hit_skills);
        state.defer_hit_passive_skills(action_scope.as_deref(), hit_skills.into_ordered());
        drain_nested_queue(
            managers,
            pool,
            catalog,
            determinism,
            hit_buff_acts.into_ordered().into(),
            result,
            state,
        )?;
    }
    let mut after_publish = reactions.after_publish;
    let after_settlement = reactions.after_settlement;
    if releases_hit_passive_skills {
        let mut released_hit_passives = state
            .take_attacked_skills(action_scope.as_ref())
            .unwrap_or_default();
        if let Some(thresholds) = state.take_hp_thresholds(action_scope.as_ref()) {
            released_hit_passives.extend(thresholds);
        }
        if let Some(hit_passives) = state.take_hit_passive_skills(action_scope.as_ref()) {
            released_hit_passives.extend(hit_passives);
        }
        if let Some(hit_skills) = state.take_hit_skills(action_scope.as_ref()) {
            released_hit_passives.extend(hit_skills);
        }
        if let Some(observers) = state.take_before_after_hit_observers(action_scope.as_ref()) {
            released_hit_passives.extend(observers);
        }
        after_publish.splice(0..0, released_hit_passives);
    }
    if releases_after_hit_skills {
        let hit_skills = state
            .take_hit_skills(action_scope.as_ref())
            .unwrap_or_default();
        let observers = state
            .take_after_hit_skills(action_scope.as_ref())
            .unwrap_or_default();
        let shell = state
            .take_after_hit_shell(action_scope.as_ref())
            .unwrap_or_default();
        after_publish.extend(hit_skills);
        after_publish.extend(observers);
        after_publish.extend(shell);
    }
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
                    && queued.frame_path.as_ref() == Some(&frame_path)
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
            push_change(
                &mut result.frames,
                action_scope,
                crate::engine::runtime::change::BattleChange::Death(death.transition()),
            );
        }
        cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
    } else if let Some(action_scope) = active_action_scope {
        state.record_deaths(action_scope.clone(), settled_deaths.iter().copied());
    } else {
        cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
        if let Some(group) = subscriber
            .as_ref()
            .and_then(|subscriber| subscriber.group.as_ref())
        {
            state.record_subscriber_deaths(group, &event_scope, settled_deaths.iter().copied());
            terminal_death = None;
        } else {
            non_action_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
        }
    }
    if releases_pending_hits && let Some(action_scope) = active_action_scope {
        let deaths = state
            .take_deaths(action_scope)
            .unwrap_or_default()
            .into_iter()
            .filter(|death| managers.hp.is_current_death(*death))
            .map(crate::engine::manager::hp::PendingDeath::transition)
            .collect::<Vec<_>>();
        for death in &deaths {
            let cleanup = queued_defeated_entity_halo_cleanup(managers, *death, action_scope);
            drain_defeated_entity_halo_cleanup(
                managers,
                pool,
                catalog,
                determinism,
                cleanup,
                result,
                state,
            )?;
            push_change(
                &mut result.frames,
                action_scope,
                crate::engine::runtime::change::BattleChange::Death(*death),
            );
        }
        terminal_death = terminal_death.or_else(|| deaths.first().copied());
        cleanup_deaths.extend(deaths);
    }
    let death_count = settled_deaths.len() + event_grouped_deaths.len();
    if death_count != 0
        && matches!(trigger, SkillOpTrigger::Active)
        && let Some(queued) = queue.iter_mut().find(|queued| {
            matches!(queued.trigger, SkillOpTrigger::Active)
                && queued.frame_path.as_ref() == Some(&frame_path)
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
        if let Some(cleanup) = queued_defeated_entity_halo_cleanup(managers, death, &event_scope) {
            death_settlements.push(cleanup);
        }
        death_settlements.push(queued_death_settlement(death, &event_scope));
    }
    after_publish.extend(death_settlements);
    if let Some(death) = terminal_death {
        after_publish.push(queued_terminal_settlement(
            BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            }),
            &event_scope,
        ));
    }
    let (after_action, held_casts) = if completes_action {
        let casts = state.take_after_action_casts(&frame_path);
        (state.take_after_action(&frame_path), casts)
    } else {
        (Vec::new(), Vec::new())
    };
    let held_casts = held_casts.into_iter().map(|(caster, mut queued)| {
        queued.caster_frame = caster;
        queued
    });

    // Manager-produced follow-ups re-enter the same queue. Skills marked
    // AfterCurrentAction are retained until that action closes.
    let mut immediate_followups = Vec::new();
    for (op, owner) in followups {
        let after_current_action = matches!(
            &op,
            RuleOp::Skill(invocation) if invocation.start.waits_for_current_action()
        );
        let skill_path = after_current_action
            .then(|| active_skill_scope_path(&result.frames, &frame_path))
            .flatten()
            .filter(|action_path| state.action_in_progress(action_path));
        let (followup_frame_path, followup_parent_path) = match owner {
            crate::engine::skill::behavior::registry::OutputOwner::Skill => (
                None,
                skill_path.clone().or_else(|| Some(frame_path.clone())),
            ),
            crate::engine::skill::behavior::registry::OutputOwner::Parent => {
                (Some(output_frame_path(owner, &frame_path)), None)
            }
            crate::engine::skill::behavior::registry::OutputOwner::CausingEvent
            | crate::engine::skill::behavior::registry::OutputOwner::ActionSkillReaction
            | crate::engine::skill::behavior::registry::OutputOwner::NestedLifecycleMechanic
            | crate::engine::skill::behavior::registry::OutputOwner::SetupParent
            | crate::engine::skill::behavior::registry::OutputOwner::SetupOrFinalSettlementParent =>
            {
                unreachable!("manager follow-up owner must be resolved")
            }
        };
        let queued = QueuedOp {
            op,
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: followup_frame_path,
            parent_path: followup_parent_path,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: deferred_followup_owner.clone(),
            subscriber: subscriber.clone(),
            caster_frame: None,
        };
        if let Some(skill_path) = skill_path {
            let caster = deferred_followup_owner
                .is_none()
                .then(|| casting_reaction(&result.frames, &skill_path, &frame_path))
                .flatten();
            state.push_after_action_cast(skill_path, caster, queued);
        } else {
            immediate_followups.push(queued);
        }
    }
    prepend(queue, immediate_followups);
    insert_after_frame(queue, &frame_path, reactions.after_skill);
    if completes_action {
        insert_after_frame(
            queue,
            &frame_path,
            [QueuedOp {
                op: RuleOp::ImmediateReservePromotion,
                trigger: SkillOpTrigger::Active,
                skill_execution: None,
                frame_path: Some(frame_path.clone()),
                parent_path: None,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber: None,
                caster_frame: None,
            }],
        );
    }
    // Buff acts waiting on the action, then held casts, follow its own reactions.
    insert_after_frame(queue, &frame_path, after_action);
    insert_after_frame(queue, &frame_path, held_casts);
    prepend(queue, after_publish);

    Ok(())
}
