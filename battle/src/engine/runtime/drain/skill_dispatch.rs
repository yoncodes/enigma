use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn drain_skill(
    managers: &mut BattleManagers,
    base_pool: &TargetPool,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
    result: &mut DrainResult,
    state: &mut DrainState,
    invocation: crate::engine::skill::action::SkillInvocation,
    trigger: SkillOpTrigger,
    skill_execution: Option<SkillExecution>,
    frame_path: Option<FramePath>,
    parent_path: Option<FramePath>,
    frame_group: Option<Rc<RefCell<Option<FramePath>>>>,
    frame_owner: Option<FrameOwner>,
    subscriber: Option<SubscriberContext>,
    caster_frame: Option<FrameOwner>,
) -> Result<(), DrainError> {
    let active_cast = matches!(trigger, SkillOpTrigger::Active)
        || matches!(&frame_owner, Some(FrameOwner::BuffAct { .. }));
    // After terminal commitment, skip active skills that have not
    // started a frame. Existing-frame continuations may finish settling.
    if active_cast
        && frame_path.is_none()
        && managers.terminal_outcome().is_some()
        && !invocation.mode.finishes_selected_conduit_phase()
    {
        cancel_invocation_progress(managers, state, None, &invocation);
        return Ok(());
    }
    let attack_has_no_target = active_cast
        && attack_has_no_target(&invocation, catalog, pool, managers, determinism, context);
    if active_cast
        && base_pool.team_type(invocation.plan.source_uid).is_some()
        && ((frame_path.is_none()
            && base_pool.entity(invocation.plan.source_uid).is_some()
            && pool.entity(invocation.plan.source_uid).is_none())
            || attack_has_no_target)
    {
        cancel_invocation_progress(managers, state, frame_path.as_ref(), &invocation);
        return Ok(());
    }
    if reaction_already_fired(state, &invocation) {
        return Ok(());
    }

    let parent_path = match caster_frame {
        Some(caster) => Some(ensure_frame(
            &mut result.frames,
            None,
            parent_path.as_deref(),
            caster,
            &trigger,
        )),
        None => parent_path,
    };
    // A skill emitted by a buff act remains nested under that buff-act
    // frame; the skill still runs through the normal skill emitter.
    let (frame_path, parent_path, frame_owner, skill_from_buff_act) =
        if let Some(owner @ FrameOwner::BuffAct { .. }) = frame_owner {
            let buff_path = ensure_frame(
                &mut result.frames,
                frame_path,
                parent_path.as_deref(),
                owner,
                &trigger,
            );
            if let Some(group) = &frame_group {
                *group.borrow_mut() = Some(buff_path.clone());
            }
            (None, Some(buff_path), None, true)
        } else {
            (frame_path, parent_path, frame_owner, false)
        };
    let trigger = if skill_from_buff_act {
        SkillOpTrigger::Active
    } else {
        trigger
    };
    let frame_path = ensure_frame(
        &mut result.frames,
        frame_path,
        parent_path.as_deref(),
        frame_owner.unwrap_or(FrameOwner::Skill {
            source_uid: invocation.plan.source_uid,
            skill_id: invocation.plan.skill_id,
            card_index: invocation.card_index,
            target_uid: invocation_frame_target(invocation.target, &trigger),
        }),
        &trigger,
    );
    if matches!(trigger, SkillOpTrigger::Event(_))
        && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
        && let FrameOwner::Skill {
            source_uid,
            skill_id,
            ..
        } = owner_at_path(&result.frames, &frame_path)
    {
        state.action_skill_reaction_frame(&action_path, *source_uid, *skill_id, &frame_path);
    }
    if !skill_from_buff_act && let Some(group) = &frame_group {
        *group.borrow_mut() = Some(frame_path.clone());
    }
    if invocation.phase.is_none()
        && matches!(trigger, SkillOpTrigger::Active)
        && frame_path.len() > 1
        && matches!(
            owner_at_path(&result.frames, &frame_path[..frame_path.len() - 1]),
            FrameOwner::Skill { .. }
        )
    {
        crate::engine::runtime::record::push_frame_cue(
            &mut result.frames,
            &frame_path,
            crate::engine::runtime::record::RoundCue::CastStep,
        );
    }
    let mut defeated_owner_card_cleanups = Vec::new();
    if matches!(trigger, SkillOpTrigger::Active)
        && invocation.phase == Some(crate::engine::skill::action::SkillPhase::AfterDamage)
        && !skill_execution
            .as_ref()
            .is_some_and(|execution| execution.resumes_phase())
        && let Some(deaths) = state.take_deaths(&frame_path)
    {
        let deaths = deaths
            .into_iter()
            .filter(|death| managers.hp.is_current_death(*death))
            .map(crate::engine::manager::hp::PendingDeath::transition)
            .collect::<Vec<_>>();
        for death in deaths {
            let cleanup = queued_defeated_entity_halo_cleanup(managers, death, &frame_path);
            drain_defeated_entity_halo_cleanup(
                managers,
                pool,
                catalog,
                determinism,
                cleanup,
                result,
                state,
            )?;
            push_death(&mut result.frames, &frame_path, managers, death);
            defeated_owner_card_cleanups.push(death);
        }
    }
    let mut execution = skill_execution.unwrap_or_else(|| SkillExecution::new(context));
    if matches!(trigger, SkillOpTrigger::Event(_)) {
        execution.set_causing_skill_target(
            active_skill_target_uid(&result.frames, &frame_path).unwrap_or_default(),
        );
    }
    if matches!(trigger, SkillOpTrigger::Active)
        && let Some(additional_count) = state.take_target_modifier(&frame_path)
    {
        execution.add_skill_targets(additional_count);
    }
    if matches!(trigger, SkillOpTrigger::Active)
        && let Some(injuries) = state.injuries(&frame_path)
    {
        let source_is_attacker = pool.source_is_attacker(invocation.plan.source_uid);
        execution.record_injuries(
            injuries
                .iter()
                .copied()
                .filter(|target_uid| pool.source_is_attacker(*target_uid) == source_is_attacker),
        );
    }
    if invocation.mode == crate::engine::skill::action::SkillExecutionMode::DirectBig {
        execution.prepare_direct_big(invocation.additional_moxie);
    }

    release_invocation_progress(managers, &invocation);
    // Skill evaluation emits RuleOps only. Managers remain the sole
    // owners of durable mutations when those operations are drained.
    let emission = skill::emit_ops(
        invocation.clone(),
        managers,
        pool,
        catalog,
        determinism,
        &mut execution,
        &trigger,
    )?;
    let configured_target_replaces_selection = matches!(trigger, SkillOpTrigger::Active)
        && execution.configured_target_replaces(
            invocation.target,
            invocation.plan.source_uid,
            pool,
        );
    let preserves_selected_frame_target = execution.preserves_selected_frame_target(
        invocation.target,
        invocation.plan.source_uid,
        pool,
    );
    for &(slot_index, condition_key) in &emission.fired_rules {
        managers.mark_rule_fired(
            invocation.plan.source_uid,
            invocation.plan.skill_id,
            catalog.rule_limit_owner(invocation.plan.skill_id, slot_index),
        );
        mark_reaction_fired(state, &invocation, slot_index, condition_key);
    }
    set_skill_target(
        &mut result.frames,
        &frame_path,
        emission.target_uid,
        configured_target_replaces_selection
            || matches!(trigger, SkillOpTrigger::Active)
                && invocation.phase == Some(SkillPhase::Damage)
                && !preserves_selected_frame_target,
    );
    // Dispatch settles the action mode, so open the action from what it emitted.
    let completes_action = emission
        .continuation
        .as_ref()
        .is_some_and(|continuation| continuation.mode.completes_action())
        || emission.ops.iter().any(|emitted| {
            matches!(
                emitted.op,
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(_)
                )
            )
        });
    if completes_action {
        state.open_action(frame_path.clone());
    }
    let mut outputs = defeated_owner_card_cleanups
        .into_iter()
        .filter_map(|death| {
            queued_defeated_owner_card_cleanup(
                pool,
                managers,
                death,
                &frame_path,
                state.defender_card_cleanup_owner(),
            )
        })
        .collect::<Vec<_>>();
    for emission in emission.ops {
        let skill::SkillEmissionOp {
            op,
            owner,
            consequence,
            frame_owner,
        } = emission;
        let op = attach_buff_grant_relation(op, consequence);
        if let RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(event),
        ) = &op
            && let Some(completion) =
                take_conduit_activation_completion(queue, event.source_uid, event.skill_id)
        {
            outputs.push(completion);
        }
        match op {
            RuleOp::Skill(child) => {
                let after_current_action = child.start.waits_for_current_action();
                let mut child_execution = SkillExecution::new(context);
                if !after_current_action {
                    child_execution.inherit_assassination_marks(&execution);
                }
                let mut queued = QueuedOp {
                    op: RuleOp::Skill(child),
                    trigger: SkillOpTrigger::Active,
                    skill_execution: Some(child_execution),
                    frame_path: None,
                    parent_path: Some(frame_path.clone()),
                    frame_group: None,
                    independent_parent_group: None,
                    frame_owner: None,
                    subscriber: subscriber.clone(),
                    caster_frame: None,
                };
                // Like manager follow-ups, wait for the enclosing action; with no
                // action in progress the cast runs now.
                match after_current_action
                    .then(|| active_skill_scope_path(&result.frames, &frame_path))
                    .flatten()
                    .filter(|action_path| state.action_in_progress(action_path))
                {
                    Some(action_path) => {
                        let caster =
                            casting_reaction(&result.frames, &action_path, &frame_path);
                        queued.parent_path = Some(action_path.clone());
                        state.push_after_action_cast(action_path, caster, queued)
                    }
                    None => outputs.push(queued),
                }
            }
            command => outputs.push(match frame_owner {
                Some(frame_owner) => QueuedOp {
                    op: command,
                    trigger: trigger.clone(),
                    skill_execution: None,
                    frame_path: None,
                    parent_path: Some(frame_path.clone()),
                    frame_group: None,
                    independent_parent_group: None,
                    frame_owner: Some(frame_owner),
                    subscriber: subscriber.clone(),
                    caster_frame: None,
                },
                None => QueuedOp {
                    op: command,
                    trigger: trigger.clone(),
                    skill_execution: None,
                    frame_path: Some(if owner
                        == crate::engine::skill::behavior::registry::OutputOwner::ActionSkillReaction
                    {
                        let action_path = active_skill_scope_path(
                            &result.frames,
                            &frame_path,
                        )
                        .ok_or(DrainError::MissingActiveSkillContext)?;
                        let FrameOwner::Skill {
                            source_uid,
                            skill_id,
                            ..
                        } = owner_at_path(&result.frames, &frame_path)
                        else {
                            return Err(DrainError::MissingActiveSkillContext);
                        };
                        state.action_skill_reaction_frame(
                            &action_path,
                            *source_uid,
                            *skill_id,
                            &frame_path,
                        )
                    } else {
                        output_frame_path(owner, &frame_path)
                    }),
                    parent_path: None,
                    frame_group: None,
                    independent_parent_group: None,
                    frame_owner: None,
                    subscriber: subscriber.clone(),
                    caster_frame: None,
                },
            }),
        }
    }
    if let Some(continuation) = emission.continuation {
        outputs.push(QueuedOp {
            op: RuleOp::Skill(continuation),
            trigger,
            skill_execution: Some(execution),
            frame_path: Some(frame_path.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: subscriber.clone(),
            caster_frame: None,
        });
    }
    prepend(queue, outputs);

    Ok(())
}
