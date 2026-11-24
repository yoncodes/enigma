use super::*;

pub(super) fn emit(
    invocation: &SkillInvocation,
    context: &EmissionContext<'_>,
    prepared: &Prepared<'_>,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
    mut outputs: Vec<SkillEmissionOp>,
) -> Result<ControlFlow<SkillEmission, SlotOutput>, SkillOpError> {
    let EmissionContext {
        managers,
        pool,
        catalog,
        trigger,
    } = *context;
    let Prepared {
        effect,
        active_phase,
        resume_slot,
        resume_target,
        source_team,
        has_row_damage,
        ..
    } = *prepared;
    let mut fired_rules = Vec::new();
    let action_start_managers = execution.action_start_managers.clone();
    for (slot_index, slot) in effect.slots.iter().enumerate().skip(resume_slot) {
        if execution.immediate_source_death()
            && matches!(
                active_phase,
                Some(SkillPhase::AfterDamage | SkillPhase::HitPassives | SkillPhase::AfterHit)
            )
        {
            continue;
        }
        if invocation
            .condition_slot
            .is_some_and(|selected| selected != slot_index)
        {
            continue;
        }
        let definition = behavior::registry::find(&slot.behavior).ok_or_else(|| {
            SkillOpError::UnregisteredBehavior {
                opcode: slot.behavior.spec.key.opcode,
                type_name: slot.behavior.spec.key.type_name.clone(),
            }
        })?;
        if !(definition.supports_slot)(slot) {
            return Err(SkillOpError::MissingBehaviorOp {
                skill_id: invocation.plan.skill_id,
                key: definition.key,
            });
        }
        let managers = if attack_modifier_reads_action_start(slot) {
            action_start_managers.as_deref().unwrap_or(managers)
        } else {
            managers
        };
        if active_phase.is_some() && definition.attack_modifier_only && slot.target_from_condition {
            continue;
        }
        if let Some(required_event) = definition.required_event {
            let SkillOpTrigger::Event(event) = trigger else {
                continue;
            };
            if !event
                .subscription_kinds()
                .any(|published| published == required_event)
            {
                continue;
            }
        }
        if let Some(phase) = active_phase {
            let routed_phases =
                slot.active_phases()
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?;
            if if routed_phases.is_empty() {
                definition.phase != phase
                    || invocation.mode.completes_action()
                        && slot
                            .completion()
                            .map_err(|route| SkillOpError::UncompiledRoute {
                                skill_id: invocation.plan.skill_id,
                                route,
                            })?
                            .is_some()
            } else {
                !routed_phases.contains(&phase)
            } {
                continue;
            }
        }
        let (conditions, selected_event, condition_key) = match (invocation.condition_key, trigger)
        {
            (None, SkillOpTrigger::Active) => {
                let phase = invocation.phase.unwrap_or(SkillPhase::Immediate);
                let phase_driver = slot
                    .compiled_subscriptions()
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?
                    .into_iter()
                    .find(|key| {
                        key.event == crate::engine::event::kind::EventKind::SkillAction
                            && key.phase.is_none_or(|driver_phase| driver_phase == phase)
                    });
                let condition_key = phase_driver.as_ref().map(|driver| driver.definition);
                (
                    phase_driver.as_ref().map_or_else(
                        || slot.conditions.clone(),
                        |driver| satisfied_conditions(&slot.conditions, driver.definition),
                    ),
                    None,
                    condition_key,
                )
            }
            (None, SkillOpTrigger::Event(_)) => (slot.conditions.clone(), None, None),
            (Some(condition_key), SkillOpTrigger::Event(event)) => {
                let subscriptions = slot.compiled_subscriptions().map_err(|route| {
                    SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    }
                })?;
                let Some(subscription) = subscriptions.iter().find(|key| {
                    key.definition == condition_key
                        && event
                            .subscription_kinds()
                            .any(|published| published == key.event)
                        && key.phase == invocation.phase
                }) else {
                    continue;
                };
                let conditions = if subscription.event.is_entry() {
                    slot.entry_conditions(&subscriptions, event)
                } else {
                    None
                };
                (
                    conditions
                        .unwrap_or_else(|| satisfied_conditions(&slot.conditions, condition_key)),
                    Some(event),
                    Some(condition_key),
                )
            }
            (None, SkillOpTrigger::Setup { stage, priority }) => {
                let keys = slot
                    .compiled_setup_keys(*stage, *priority)
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?;
                let Some(&key) = keys.first() else {
                    continue;
                };
                (satisfied_conditions(&slot.conditions, key), None, Some(key))
            }
            (Some(condition_key), SkillOpTrigger::Setup { stage, priority }) => {
                let keys = slot
                    .compiled_setup_keys(*stage, *priority)
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?;
                if !keys.contains(&condition_key) {
                    continue;
                }
                (
                    satisfied_conditions(&slot.conditions, condition_key),
                    None,
                    Some(condition_key),
                )
            }
            _ => continue,
        };
        if condition_key
            .and_then(|condition_key| {
                crate::engine::skill::condition::registry::find_key(
                    condition_key.opcode,
                    condition_key.type_name,
                )
            })
            .is_some_and(|definition| {
                definition.skill_action_trigger.rejects_action(
                    execution.context.active_skill_is_attack,
                    execution.context.active_skill_mode,
                    managers
                        .entity
                        .passive_source(invocation.plan.source_uid, invocation.plan.skill_id),
                    execution.context.action_damage_amount,
                    execution.context.active_skill_id == invocation.plan.skill_id,
                ) || definition
                    .skill_action_trigger
                    .rejects_extra_skill(execution.context.extra_skill_kind)
            })
        {
            continue;
        }
        if condition_key.is_some_and(|_| {
            !managers.can_fire_rule(
                invocation.plan.source_uid,
                invocation.plan.skill_id,
                catalog.rule_limit_owner(invocation.plan.skill_id, slot_index),
                slot.limit,
                slot.round_limit,
            )
        }) {
            continue;
        }
        let conditions = satisfied_card_enchants(&conditions, &invocation.card_enchants);
        let condition_targets = if slot.conditions.is_empty() {
            Vec::new()
        } else {
            TargetResolver::resolve_with_managers_and_context(
                &slot.condition_target,
                invocation.plan.skill_id,
                invocation.plan.source_uid,
                pool,
                determinism,
                Some(managers),
                execution.context,
            )
        };
        let behavior_target_order = if slot.target_from_condition {
            crate::engine::skill::condition::registry::behavior_target_order(&conditions)
        } else {
            None
        };
        let per_target_conditions = behavior_target_order.is_some();
        let fire_count = if per_target_conditions {
            1
        } else {
            let resource_count = selected_event.and_then(resource_event).and_then(|event| {
                resource_fire_count(
                    &slot.conditions,
                    invocation.plan.skill_id,
                    ResourceConditionContext {
                        event,
                        source_uid: invocation.plan.source_uid,
                        condition_targets: &condition_targets,
                        condition_target_code: slot.condition_target.code,
                        managers,
                        pool,
                        random_roll: None,
                    },
                    determinism,
                )
            });
            resource_count.unwrap_or_else(|| {
                let mut context = execution.context;
                if let Some(random) =
                    crate::engine::skill::condition::query::find(&conditions, &|condition| {
                        crate::engine::skill::condition::query::is_random(condition)
                    })
                {
                    let deterministic_match = conditions
                        .iter()
                        .filter(|condition| {
                            !crate::engine::skill::condition::query::is_random(condition)
                        })
                        .all(|condition| {
                            conditions_match(
                                std::slice::from_ref(condition),
                                invocation.plan.source_uid,
                                &condition_targets,
                                Some(managers),
                                pool,
                                context,
                            )
                        });
                    if !deterministic_match {
                        return 0;
                    }
                    context.condition_random_roll = Some(determinism.condition_random_roll(
                        invocation.plan.skill_id,
                        invocation.plan.source_uid,
                        random.opcode,
                    ));
                }
                conditions_fire_count(
                    &conditions,
                    invocation.plan.source_uid,
                    &condition_targets,
                    Some(managers),
                    pool,
                    context,
                )
            })
        };
        let fire_count = (definition.resolve_fire_count)(
            behavior::registry::BehaviorFireCountContext {
                managers,
                source_team,
            },
            &slot.behavior,
            fire_count,
        );
        if fire_count <= 0 {
            continue;
        }
        if active_phase.is_some()
            && crate::engine::skill::rule::ownership::behavior_is_owned_by_buff_act(
                slot,
                invocation.plan.source_uid,
                managers,
            )
        {
            continue;
        }
        let consequence = consequence_policy(slot, invocation, trigger)?;
        let behavior_target_source = condition_key
            .and_then(|key| {
                crate::engine::skill::condition::registry::find_key(key.opcode, key.type_name)
            })
            .map(|definition| definition.behavior_target_source)
            .unwrap_or_default();
        let condition_uses_active_skill_targets = behavior_target_source
            == crate::engine::skill::condition::registry::BehaviorTargetSource::ActiveSkillTargets
            || crate::engine::skill::condition::registry::conditions_use_active_skill_targets(
                &conditions,
            );
        let condition_uses_hit_targets = behavior_target_source
            == crate::engine::skill::condition::registry::BehaviorTargetSource::HitTargets;
        let event_subject = (slot.target_from_condition
            && behavior_target_source
                == crate::engine::skill::condition::registry::BehaviorTargetSource::EventSubject
            && execution.context.runtime_target_uid != 0)
            .then_some(execution.context.runtime_target_uid);
        let child_skill_kind = condition_key
            .and_then(|condition_key| {
                crate::engine::skill::condition::registry::find_key(
                    condition_key.opcode,
                    condition_key.type_name,
                )
            })
            .and_then(|definition| definition.skill_action_trigger.child_skill_kind());
        let child_target_source = condition_key
            .and_then(|condition_key| {
                crate::engine::skill::condition::registry::find_key(
                    condition_key.opcode,
                    condition_key.type_name,
                )
            })
            .map(|definition| definition.child_target_source)
            .unwrap_or_default();
        let uses_action_targets = uses_action_targets(
            slot,
            condition_uses_active_skill_targets || condition_uses_hit_targets,
        );
        let event_targets = if condition_uses_hit_targets && slot.target.code != SOURCE_TARGET_CODE
        {
            selected_event.and_then(active_skill_hit_targets)
        } else if condition_uses_active_skill_targets && uses_action_targets {
            selected_event.and_then(active_skill_targets)
        } else {
            None
        };
        let targets_are_hits = condition_uses_hit_targets
            && definition.target_emission_mode != behavior::registry::TargetEmissionMode::Owner
            && (event_targets.is_some()
                || active_phase.is_some() && has_row_damage && uses_action_targets);
        let mut targets =
            if definition.target_emission_mode == behavior::registry::TargetEmissionMode::Owner {
                vec![invocation.plan.source_uid]
            } else if let Some(subject_uid) = event_subject {
                vec![subject_uid]
            } else if let Some(targets) = event_targets {
                targets.to_vec()
            } else if active_phase.is_some()
                && has_row_damage
                && condition_uses_hit_targets
                && uses_action_targets
            {
                execution.attacked_targets.clone()
            } else if active_phase.is_some()
                && uses_action_targets
                && let Some(targets) = execution.configured_targets.as_ref()
            {
                targets.clone()
            } else {
                behavior::use_skill::resolve_targets(
                    invocation.plan.skill_id,
                    invocation.plan.source_uid,
                    slot.target.code,
                    pool,
                    determinism,
                    &slot.behavior,
                )
                .unwrap_or_else(|| {
                    TargetResolver::resolve_with_managers_and_context(
                        &slot.target,
                        invocation.plan.skill_id,
                        invocation.plan.source_uid,
                        pool,
                        determinism,
                        Some(managers),
                        execution.context,
                    )
                })
            };
        if targets_are_hits && invocation.phase == Some(SkillPhase::AfterDamage) {
            targets.retain(|target_uid| managers.hp.current(*target_uid) > 0);
        }
        if let Some(target_order) = behavior_target_order {
            TargetResolver::order_behavior_candidates(&mut targets, target_order, managers);
            targets.retain(|target_uid| {
                conditions.iter().all(|condition| {
                    conditions_match(
                        std::slice::from_ref(condition),
                        invocation.plan.source_uid,
                        std::slice::from_ref(target_uid),
                        Some(managers),
                        pool,
                        execution.context,
                    )
                })
            });
        }
        if definition.target_emission_mode == behavior::registry::TargetEmissionMode::Once {
            targets.truncate(1);
        }
        super::super::trace::targets(
            invocation,
            slot,
            trigger,
            execution.context,
            managers,
            pool,
            &targets,
        );
        let outputs_before = outputs.len();
        let target_count = targets.len();
        for (target_index, target_uid) in targets.iter().copied().enumerate().skip(resume_target) {
            let target_outputs_before = outputs.len();
            if !has_row_damage && target_uid != 0 {
                execution.primary_target_uid.get_or_insert(target_uid);
            }
            let (emissions, transfer_count) = match definition.fire_count_mode {
                behavior::registry::FireCountMode::Repeat => (fire_count, 1),
                behavior::registry::FireCountMode::Transfer => (1, fire_count),
            };
            for _ in 0..emissions {
                let mut behavior_ops = behavior::registry::emit_runtime_ops(
                    definition,
                    BehaviorOpContext {
                        source_uid: invocation.plan.source_uid,
                        source_team,
                        target_uid,
                        active_skill_id: invocation.plan.skill_id,
                        transfer_count,
                        event: selected_event,
                        managers,
                        pool,
                        determinism,
                        modifiers: &mut execution.modifiers,
                        target: &mut execution.context,
                    },
                    &slot.behavior,
                    catalog,
                )
                .ok_or(SkillOpError::MissingBehaviorOp {
                    skill_id: invocation.plan.skill_id,
                    key: definition.key,
                })?;
                if child_target_source
                    == crate::engine::skill::condition::registry::ChildTargetSource::Event
                    && let Some(target_uid) = selected_event.and_then(|event| event.target_uid())
                {
                    for op in &mut behavior_ops {
                        if let RuleOp::Skill(child) = op {
                            child.target = SkillTarget::Explicit(target_uid);
                        }
                    }
                }
                outputs.extend(behavior_ops.into_iter().enumerate().map(|(index, op)| {
                    let owner = (definition.output_owner_for)(&slot.behavior, &op, index)
                        .unwrap_or(definition.output_owner)
                        .resolve(
                            match trigger {
                                SkillOpTrigger::Event(event) => Some(event.kind()),
                                _ => None,
                            },
                            matches!(trigger, SkillOpTrigger::Setup { .. }),
                        );
                    SkillEmissionOp {
                        op,
                        owner,
                        consequence,
                        frame_owner: None,
                    }
                }));
            }
            if definition.target_emission_mode
                == behavior::registry::TargetEmissionMode::EachSettled
                && outputs.len() > target_outputs_before
                && target_index + 1 < target_count
            {
                if let Some(child_skill_kind) = child_skill_kind {
                    for emission in outputs.iter_mut().skip(target_outputs_before) {
                        if let RuleOp::Skill(child) = &mut emission.op {
                            child.extra_skill_kind = Some(child_skill_kind);
                        }
                    }
                }
                execution.resume_slot = slot_index;
                execution.resume_target = target_index + 1;
                let mut continuation = invocation.clone();
                continuation.phase = active_phase;
                continuation.additional_moxie = execution.context.additional_moxie;
                return Ok(ControlFlow::Break(SkillEmission {
                    ops: outputs,
                    fired_rules,
                    continuation: Some(continuation),
                    target_uid: execution.primary_target_uid,
                }));
            }
        }
        if let Some(child_skill_kind) = child_skill_kind {
            for emission in outputs.iter_mut().skip(outputs_before) {
                if let RuleOp::Skill(child) = &mut emission.op {
                    child.extra_skill_kind = Some(child_skill_kind);
                }
            }
        }
        if outputs.len() > outputs_before
            && let Some(condition_key) = condition_key
        {
            fired_rules.push((slot_index, condition_key));
        }
        // The game runs a skill's slots in order, so a later condition in this phase sees what
        // this slot changed: let those changes commit before checking it.
        if let Some(phase) = active_phase
            && outputs.len() > outputs_before
            && later_condition_runs_in_phase(
                &effect.slots[slot_index + 1..],
                phase,
                invocation.plan.skill_id,
            )?
        {
            if execution.action_start_managers.is_none()
                && later_attack_modifier_runs_in_phase(
                    &effect.slots[slot_index + 1..],
                    phase,
                    invocation.plan.skill_id,
                )?
            {
                execution.action_start_managers = Some(std::sync::Arc::new(managers.clone()));
            }
            execution.resume_slot = slot_index + 1;
            let mut continuation = invocation.clone();
            continuation.phase = Some(phase);
            continuation.additional_moxie = execution.context.additional_moxie;
            return Ok(ControlFlow::Break(SkillEmission {
                ops: outputs,
                fired_rules,
                continuation: Some(continuation),
                target_uid: execution.primary_target_uid,
            }));
        }
    }
    Ok(ControlFlow::Continue(SlotOutput {
        ops: outputs,
        fired_rules,
    }))
}

fn later_condition_runs_in_phase(
    slots: &[SkillEffectSlot],
    phase: SkillPhase,
    skill_id: i32,
) -> Result<bool, SkillOpError> {
    for slot in slots.iter().filter(|slot| !slot.conditions.is_empty()) {
        if slot_runs_in_phase(slot, phase, skill_id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn consequence_policy(
    slot: &SkillEffectSlot,
    invocation: &SkillInvocation,
    trigger: &SkillOpTrigger,
) -> Result<ConsequencePolicy, SkillOpError> {
    let Some(condition_key) = invocation.condition_key else {
        return Ok(ConsequencePolicy::Default);
    };
    let route = slot
        .compiled_route
        .as_ref()
        .map_err(|route| SkillOpError::UncompiledRoute {
            skill_id: invocation.plan.skill_id,
            route: route.clone(),
        })?;
    let mut keys = route
        .branches
        .iter()
        .filter_map(|branch| match (branch.driver, trigger) {
            (Some(ConditionDriver::Trigger(driver)), SkillOpTrigger::Event(event))
                if driver.key == condition_key
                    && driver.event == event.kind()
                    && driver.phase == invocation.phase =>
            {
                Some(driver.key)
            }
            (Some(ConditionDriver::Setup(driver)), SkillOpTrigger::Setup { stage, priority })
                if driver.key == condition_key
                    && driver.stage == *stage
                    && driver.priority == *priority =>
            {
                Some(driver.key)
            }
            _ => None,
        });
    let Some(key) = keys.next() else {
        return Ok(ConsequencePolicy::Default);
    };
    if keys.any(|candidate| candidate != key) {
        return Err(SkillOpError::AmbiguousConditionDriver {
            skill_id: invocation.plan.skill_id,
            opcode: condition_key.opcode,
        });
    }
    Ok(
        crate::engine::skill::condition::registry::find_key(key.opcode, key.type_name)
            .map(|definition| definition.consequence)
            .unwrap_or_default(),
    )
}
