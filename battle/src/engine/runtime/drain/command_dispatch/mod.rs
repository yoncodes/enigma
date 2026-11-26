use crate::engine::{
    event::kind::EventKind,
    manager::{
        buff::{BuffFanoutResult, BuffReplacementParts},
        hp::PendingDeath,
    },
    skill::behavior::registry::OutputOwner,
};

use super::{command_prepare::prepare_command, *};

mod commit;
mod death;
mod heal_attribution;
mod publication;
mod team_buff_layers;

struct DrainAccess<'a> {
    managers: &'a mut BattleManagers,
    pool: &'a TargetPool,
    catalog: &'a SkillEffectCatalog,
    determinism: &'a mut RoundDeterminism,
    result: &'a mut DrainResult,
    state: &'a mut DrainState,
}

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
    let loss_group_cast = state.round_start_loss_casts.is_some()
        && super::round_start_loss::is_threshold_from_loss(&command);
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
        let parent = super::overflow_frame::storage_scope(
            &result.frames,
            parent_path.as_deref(),
            &trigger,
            &command,
        )
        .or(parent_path.as_deref());
        ensure_frame(
            &mut result.frames,
            frame_path,
            parent,
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
    let commit::Committed {
        mut outcome,
        followups,
        claimed_counters,
        entry_halo_fanout,
    } = commit::apply(
        DrainAccess {
            managers: &mut *managers,
            pool,
            catalog,
            determinism: &mut *determinism,
            result: &mut *result,
            state: &mut *state,
        },
        commit::Scope {
            base_pool,
            frame_path: &frame_path,
            action_scope: action_scope.as_ref(),
            trigger: &trigger,
        },
        queue,
        bus,
        command,
    )?;
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
    let layer_halo_fanout_output_owner = if !entry_halo_fanout
        && current_skill.is_some()
        && matches!(trigger, SkillOpTrigger::Setup { .. })
        && event_scope == frame_path
    {
        crate::engine::skill::behavior::registry::OutputOwner::Parent
    } else {
        fanout_output_owner
    };

    // Managers publish semantic events through the bus. Reactions are
    // derived from these committed events, never fired by packet code.
    let mut events = std::iter::from_fn(|| bus.pop()).collect::<Vec<_>>();
    let team_layer_settlements = managers.buff.settle_team_buff_layers(&events, catalog);
    let mut event_groups = match &outcome {
        RuleOutcome::HpBatch(batch) => grouped_hp_events(
            &events,
            batch.iter().map(|execution| execution.events()).collect(),
        ),
        _ => vec![events.clone()],
    };
    heal_attribution::attach(
        &mut events,
        &mut event_groups,
        current_skill.map(|(_, id, _)| id),
        &trigger,
    );
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
        let results = match &outcome {
            RuleOutcome::Buff(changes) => std::slice::from_ref(changes.as_ref()),
            RuleOutcome::BuffBatch(changes) => changes.as_slice(),
            _ => &[],
        };
        for burn in results
            .iter()
            .filter_map(|changes| changes.burn_application)
        {
            if burn.source_uid == source_uid {
                execution.record_burn(burn);
            }
        }
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

    let publication_scope = publication::Scope {
        context,
        trigger: &trigger,
        frame_path: &frame_path,
        event_scope: &event_scope,
        action_scope: action_scope.as_ref(),
        entry_halo_scope: &entry_halo_scope,
        field_transfer_owner: match &trigger {
            SkillOpTrigger::Event(event) if event.kind() == EventKind::SmallRoundEnd => subscriber
                .as_ref()
                .and_then(|subscriber| subscriber.settlement_owner.as_deref()),
            _ => None,
        },
        field_level_grant: super::field_level::command_grant(&trigger, subscriber.as_ref()),
        current_skill,
        fanout_output_owner,
        layer_halo_fanout_output_owner,
    };
    let publication::before::PreparedEvents {
        replacement,
        event_groups,
        started_after_reactions,
        released_hit_events,
        releases_pending_hits,
        releases_hit_passive_skills,
        releases_after_hit_skills,
    } = publication::before::prepare(
        DrainAccess {
            managers: &mut *managers,
            pool,
            catalog,
            determinism: &mut *determinism,
            result: &mut *result,
            state: &mut *state,
        },
        publication_scope,
        pending_hits,
        &outcome,
        &fanout,
        &events,
        event_groups,
    )?;

    // Once pre-publication work is complete, record the authoritative
    // outcome and any buff fanout beneath their semantic owners.
    let settles_death_immediately = outcome.settles_death_immediately();
    let pending_deaths = outcome.take_deaths();
    let settlement_action_scope = action_scope
        .as_ref()
        .filter(|path| matches!(trigger, SkillOpTrigger::Active) || state.action_in_progress(path));
    let active_action_scope = (!settles_death_immediately)
        .then_some(settlement_action_scope)
        .flatten();
    let original_pending_deaths = pending_deaths.clone();
    publication::record(
        DrainAccess {
            managers: &mut *managers,
            pool,
            catalog,
            determinism: &mut *determinism,
            result: &mut *result,
            state: &mut *state,
        },
        publication_scope,
        &outcome,
        replacement.as_ref(),
        &events,
        fanout,
    )?;
    let was_skill_action_started = matches!(
        &outcome,
        RuleOutcome::SkillActionStarted { .. }
            | RuleOutcome::ActionCostPaid(_)
            | RuleOutcome::PaidSkillStarted(_)
    );
    let was_hp_batch = matches!(&outcome, RuleOutcome::HpBatch(_));
    result.outcomes.push(outcome);

    let terminal_owner_uids = managers
        .terminal_outcome()
        .and_then(|outcome| outcome.winning_team())
        .map(|winning_team| base_pool.team_uids(winning_team));

    let reactions = publication::dispatch_after(
        DrainAccess {
            managers: &mut *managers,
            pool,
            catalog,
            determinism: &mut *determinism,
            result: &mut *result,
            state: &mut *state,
        },
        publication_scope,
        publication::ReleasedEvents {
            started_after_reactions,
            event_groups: &event_groups,
            team_layer_settlements,
            released_hit_events: released_hit_events.as_deref(),
            terminal_owner_uids: terminal_owner_uids.as_deref(),
            pending_deaths: active_action_scope
                .is_some()
                .then_some(pending_deaths.as_slice()),
            was_skill_action_started,
            was_hp_batch,
        },
    )?;
    let mut after_publish = reactions.after_publish;
    let after_settlement = reactions.after_settlement;
    if releases_hit_passive_skills {
        if let Some(path) = action_scope.as_deref() {
            state.reactions(path).hit_passives_released = true;
        }
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
    let after_publish = death::settle(
        DrainAccess {
            managers: &mut *managers,
            pool,
            catalog,
            determinism: &mut *determinism,
            result: &mut *result,
            state: &mut *state,
        },
        death::Scope {
            trigger: &trigger,
            frame_path: &frame_path,
            event_scope: &event_scope,
            current_skill,
            subscriber: subscriber.as_ref(),
        },
        death::Pending {
            pending_deaths,
            original_pending_deaths,
            settles_death_immediately,
            settlement_action_scope,
            active_action_scope,
            releases_pending_hits,
        },
        queue,
        after_publish,
        after_settlement,
    )?;
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
            | crate::engine::skill::behavior::registry::OutputOwner::InitialSovereignBoost
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
        } else if loss_group_cast && let Some(casts) = &mut state.round_start_loss_casts {
            casts.push_back(QueuedOp {
                parent_path: None,
                ..queued
            });
        } else {
            immediate_followups.push(queued);
        }
    }
    prepend(queue, immediate_followups);
    super::field_level::schedule_after_skill(
        queue,
        &frame_path,
        publication_scope.field_level_grant,
        reactions.after_skill,
    );
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
    insert_after_frame(queue, &frame_path, claimed_counters);
    insert_after_frame(queue, &frame_path, held_casts);
    prepend(queue, after_publish);

    Ok(())
}
