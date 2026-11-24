use super::*;

pub(super) fn prepare<'a>(
    invocation: &mut SkillInvocation,
    context: &EmissionContext<'a>,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
) -> Result<ControlFlow<SkillEmission, (Prepared<'a>, Vec<SkillEmissionOp>)>, SkillOpError> {
    let EmissionContext {
        managers,
        pool,
        catalog,
        trigger,
    } = *context;
    invocation.extra_skill_kind = invocation.extra_skill_kind.or_else(|| {
        managers
            .entity
            .skill_kind(invocation.plan.source_uid, invocation.plan.skill_id)
            .or_else(|| {
                crate::engine::skill::condition::extra::skill_kind_from_is_extra(
                    catalog.extra_kind(invocation.plan.skill_id),
                )
            })
    });
    let effect_skill_id = if invocation.extra_skill_kind
        == Some(crate::engine::skill::condition::extra::ExtraSkillKind::Reinforced)
    {
        catalog
            .reinforced_skill(invocation.plan.skill_id)
            .unwrap_or(invocation.plan.skill_id)
    } else {
        invocation.plan.skill_id
    };
    let issues = catalog.issues(effect_skill_id);
    for issue in issues.iter().filter(|issue| {
        issue.reason == crate::engine::skill::effect::catalog::RuleIssueReason::UnsupportedBehavior
    }) {
        tracing::warn!(
            target: LOG_TARGET,
            skill_id = invocation.plan.skill_id,
            effect_id = issue.effect_id,
            slot = issue.slot,
            opcode = issue.opcode,
            type_name = issue.type_name,
            "skipping unsupported skill behavior"
        );
    }
    // Unknown mechanics are intentionally fail-open so incomplete content cannot abort the
    // client session. Broken or missing config remains fatal.
    let blocking_issues = issues
        .iter()
        .filter(|issue| issue.reason.is_blocking())
        .cloned()
        .collect::<Vec<_>>();
    if !blocking_issues.is_empty() {
        return Err(SkillOpError::InvalidSkillDefinition {
            skill_id: invocation.plan.skill_id,
            issues: blocking_issues,
        });
    }
    let effect = catalog
        .get(effect_skill_id)
        .ok_or(SkillOpError::MissingSkill(effect_skill_id))?;
    execution.context.target_observed_extra_action =
        invocation.target_observed_extra_action.unwrap_or(
            invocation.extra_skill_kind
                == Some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction),
        );
    invocation.mode = action_mode(invocation.mode, invocation.extra_skill_kind);
    if (invocation.condition_key.is_some() || invocation.condition_slot.is_some())
        && matches!(trigger, SkillOpTrigger::Active)
    {
        return Err(SkillOpError::MissingTriggerContext(
            invocation
                .condition_key
                .map(|key| key.opcode)
                .unwrap_or_default(),
        ));
    }

    execution.context.active_skill_id = invocation.plan.skill_id;
    execution.context.active_skill_source_uid = invocation.plan.source_uid;
    execution
        .context
        .action_start_ex_point
        .get_or_insert_with(|| managers.ex_point.get(invocation.plan.source_uid));
    if invocation.card_index > 0 {
        execution.context.active_card_index = invocation.card_index;
    }
    if let Some(recorded) = invocation.recorded_skill {
        execution.context.recorded_skill_id = recorded.skill_id;
        execution.context.recorded_skill_source_uid = recorded.source_uid;
    }
    execution.context.logic_target = match invocation.target {
        SkillTarget::LogicRule(code) => code,
        _ => catalog.logic_target(effect_skill_id),
    };
    execution.context.damage_target_count_kind = managers
        .catalog()
        .damage_target_count_kind(execution.context.logic_target);
    execution.context.extra_skill_kind = invocation
        .extra_skill_kind
        .map(|kind| kind.id())
        .unwrap_or_else(|| catalog.extra_kind(invocation.plan.skill_id));
    if matches!(
        invocation.mode,
        crate::engine::skill::action::SkillExecutionMode::Direct
            | crate::engine::skill::action::SkillExecutionMode::Active
            | crate::engine::skill::action::SkillExecutionMode::Synchronization
            | crate::engine::skill::action::SkillExecutionMode::DirectBig
            | crate::engine::skill::action::SkillExecutionMode::Device
            | crate::engine::skill::action::SkillExecutionMode::DeviceCard
    ) {
        execution.context.active_skill_mode = invocation.mode;
        execution.context.active_skill_is_attack = catalog.is_attack(effect_skill_id);
        if invocation.mode == crate::engine::skill::action::SkillExecutionMode::Direct
            || (matches!(trigger, SkillOpTrigger::Active) && invocation.card_index > 0)
        {
            execution.context.active_skill_slot = pool.skill_slot(
                managers,
                invocation.plan.source_uid,
                invocation.plan.skill_id,
            );
            execution.context.active_skill_rank =
                managers.catalog().skill_rank(invocation.plan.skill_id);
            // "SkillType" conditions read the skill's target kind: 1-target (1) or mass (2).
            execution.context.active_skill_type = execution.context.damage_target_count_kind;
            execution.context.active_skill_effect_tag = catalog.effect_tag(effect_skill_id);
        }
        execution.context.additional_moxie = invocation.additional_moxie;
        execution.context.direct_skill_body = true;
    }
    execution.context.card_selected_target_uid = match invocation.target {
        SkillTarget::Explicit(uid)
            if matches!(trigger, SkillOpTrigger::Active) && invocation.card_index > 0 =>
        {
            uid
        }
        _ => 0,
    };
    if let SkillTarget::Explicit(uid) = invocation.target {
        execution.context.runtime_target_uid = uid;
        execution.primary_target_uid.get_or_insert(uid);
        execution.record_targets([uid]);
    }
    if let SkillOpTrigger::Event(event) = trigger {
        apply_event_context(managers, catalog, &mut execution.context, event);
    }
    let source_team =
        pool.team_type(invocation.plan.source_uid)
            .ok_or(SkillOpError::MissingSourceEntity(
                invocation.plan.source_uid,
            ))?;
    execution.sync_team_injury_count(managers.injury.round_count(source_team));
    let blood_pool = managers
        .gauge
        .get(crate::engine::mechanic::bloodtithe::rule::key(source_team));
    execution.context.blood_pool_value = blood_pool.map(|state| state.current).unwrap_or_default();
    execution.context.blood_pool_max = blood_pool.and_then(|state| state.max).unwrap_or_default();
    let lingering_glow_key = crate::engine::mechanic::lingering_glow::key(source_team);
    execution.context.heat_scale_value = managers
        .gauge
        .get(lingering_glow_key)
        .map(|state| state.current)
        .unwrap_or_default();
    execution.context.heat_scale_raw_value = managers
        .gauge
        .raw_value(lingering_glow_key)
        .unwrap_or_else(|| execution.context.heat_scale_value.saturating_mul(1000));
    let mut outputs = Vec::new();
    let resume_slot = std::mem::take(&mut execution.resume_slot);
    let resume_target = std::mem::take(&mut execution.resume_target);
    let active_phase = if matches!(trigger, SkillOpTrigger::Active) {
        let has_source_opening_subscriber = invocation.phase.is_none()
            && crate::engine::skill::subscriber::for_compiled_events(
                pool,
                managers,
                catalog,
                [crate::engine::event::kind::EventKind::SkillAction],
            )?
            .skills
            .iter()
            .any(|subscriber| subscriber.key.phase == Some(SkillPhase::SourceOpening));
        Some(match invocation.phase {
            Some(phase) => phase,
            None if has_source_opening_subscriber
                || skill_runs_in_phase(
                    &effect.slots,
                    SkillPhase::SourceOpening,
                    invocation.plan.skill_id,
                )? =>
            {
                SkillPhase::SourceOpening
            }
            None => SkillPhase::Immediate,
        })
    } else {
        None
    };
    let resuming_phase = resume_slot > 0
        || resume_target > 0
        || (active_phase == Some(SkillPhase::Immediate) && execution.effect_started);
    if !resuming_phase
        && matches!(
            active_phase,
            Some(
                crate::engine::skill::action::SkillPhase::SourceOpening
                    | crate::engine::skill::action::SkillPhase::Immediate
            )
        )
        && execution.configured_targets.is_none()
    {
        let configured;
        let request = if let SkillTarget::LogicRule(code) = invocation.target {
            configured = TargetRequest {
                code,
                raw: Vec::new(),
            };
            &configured
        } else {
            catalog.logic_request(effect_skill_id)
        };
        let routes_configured_damage = effect
            .slots
            .iter()
            .any(|slot| behavior::routes_configured_damage(&slot.behavior));
        let needs_action_targets = effect.slots.iter().any(|slot| {
            uses_action_targets(
                slot,
                crate::engine::skill::condition::registry::conditions_use_active_skill_targets(
                    &slot.conditions,
                ),
            )
        });
        let defers_configured_targets = routes_configured_damage && !needs_action_targets;
        let configured_targets = if defers_configured_targets {
            Vec::new()
        } else {
            TargetResolver::resolve_action_targets(
                request,
                invocation.plan.skill_id,
                invocation.plan.source_uid,
                pool,
                determinism,
                Some(managers),
                execution.context,
            )
        };
        if !defers_configured_targets
            && execution.context.logic_target != SOURCE_TARGET_CODE
            && let SkillTarget::Explicit(selected) = invocation.target
            && !configured_targets.contains(&selected)
        {
            execution.primary_target_uid = None;
            execution.affected_targets.clear();
        }
        let _ = determinism.take_skill_target_choice(
            invocation.plan.skill_id,
            invocation.plan.source_uid,
            request.code,
        );
        execution.additional_damage_targets =
            selected_target_order(invocation.target, &configured_targets);
        if let Some(&main_target) = configured_targets.first() {
            execution.context.runtime_target_uid = main_target;
            execution.primary_target_uid.get_or_insert(main_target);
        }
        execution.record_targets(configured_targets.iter().copied());
        execution.configured_targets = Some(configured_targets);
    }
    if !resuming_phase && active_phase == Some(SkillPhase::Immediate) {
        if let Some(modifier) = invocation.rate_modifier {
            execution.modifiers.rates.push(modifier);
        }
        execution.modifiers.attack_attributes.extend(
            crate::engine::skill::buff_act::use_skill_modifier::attribute_deltas(
                managers,
                invocation.plan.source_uid,
                invocation.mode,
            ),
        );
        execution.modifiers.rates.extend(
            crate::engine::skill::buff_act::attack_skill_rate_up::rate_modifiers(
                managers,
                invocation.plan.source_uid,
                invocation.plan.skill_id,
                catalog.is_big_skill(invocation.plan.skill_id),
                execution.context.runtime_target_uid,
                invocation.mode,
            ),
        );
        plan::snapshot_action_start_damage_state(managers, catalog, effect_skill_id, execution)?;
        // The caster's and its targets' reactions to the action starting run before its own effects.
        let effect_started_owners = std::iter::once(invocation.plan.source_uid)
            .chain(execution.affected_targets.iter().copied())
            .collect::<Vec<_>>();
        let effect_started_subscribers =
            crate::engine::skill::subscriber::for_compiled_owner_events(
                pool,
                managers,
                catalog,
                [crate::engine::event::kind::EventKind::SkillEffectStarted],
                &effect_started_owners,
            )
            .map_err(SkillOpError::from)?;
        if !effect_started_subscribers.skills.is_empty()
            || !effect_started_subscribers.buff_acts.is_empty()
        {
            let hand_settlement = invocation.action_hand_settlement.take();
            outputs.push(effect_started_op(
                invocation,
                managers,
                catalog,
                pool,
                execution,
                hand_settlement,
            ));
            execution.effect_started = true;
            let mut continuation = invocation.clone();
            continuation.phase = active_phase;
            return Ok(ControlFlow::Break(SkillEmission {
                ops: outputs,
                fired_rules: Vec::new(),
                continuation: Some(continuation),
                target_uid: execution.primary_target_uid,
            }));
        }
    }
    if (active_phase == Some(SkillPhase::Immediate) || matches!(trigger, SkillOpTrigger::Event(_)))
        && let Some(origin) = invocation.action_hand_settlement.take()
    {
        outputs.push(SkillEmissionOp {
            op: RuleOp::ActionHandSettlement { origin },
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        });
    }
    let has_row_damage = catalog.damage_rate(effect_skill_id) > 0
        && !effect
            .slots
            .iter()
            .any(|slot| slot.behavior.spec.kind == BehaviorKind::IgnoreSkillConfigDamageRate);
    let has_healing = effect.slots.iter().any(|slot| {
        matches!(
            slot.behavior.spec.kind,
            BehaviorKind::Heal
                | BehaviorKind::HealCantCrit
                | BehaviorKind::HealByTwoAttr
                | BehaviorKind::HealOrAddBuffByHp
                | BehaviorKind::FakeHpToHeal
                | BehaviorKind::NuoDiKaDamage
        )
    });
    if (active_phase == Some(SkillPhase::Immediate) || matches!(trigger, SkillOpTrigger::Event(_)))
        && has_healing
        && !execution.healing_modifiers_collected
    {
        execution.healing_modifiers_collected = true;
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        behavior::rate::emit_active_target_attack_attributes(
            &mut modifiers,
            invocation.plan.source_uid,
            effect_skill_id,
            invocation.plan.skill_id,
            behavior::rate::RateRuntime {
                effects: catalog,
                managers,
                pool,
                context: execution.context,
            },
            determinism,
        );
        let passive_skills = pool
            .entity(invocation.plan.source_uid)
            .map(|entity| entity.passive_skills.as_slice())
            .unwrap_or_default();
        behavior::rate::emit_passive_attack_attributes(
            &mut modifiers,
            invocation.plan.source_uid,
            invocation.plan.skill_id,
            passive_skills,
            behavior::rate::RateRuntime {
                effects: catalog,
                managers,
                pool,
                context: execution.context,
            },
            determinism,
        );
        execution.modifiers.attack_attributes.extend(
            modifiers
                .attack_attributes
                .into_iter()
                .filter(|(attr, _)| *attr == crate::engine::entity::attr::AttrId::HealingDone),
        );
    }
    // An extra action's Force Field Eureka cost and forced crit open the action, before its own
    // effects; other additional-damage activations follow those effects.
    if active_phase == Some(SkillPhase::Immediate) && !execution.opening_effects_emitted {
        execution.opening_effects_emitted = true;
        for activation in plan::LinkedDamagePlan::activations(invocation, managers, execution) {
            if has_row_damage && activation.pays_extra_action_cost {
                outputs.extend(emit_additional_damage_activation(
                    invocation, execution, activation,
                ));
            }
        }
        if has_row_damage && plan::forces_critical(invocation, managers, execution) {
            outputs.push(SkillEmissionOp {
                op: RuleOp::EffectMarker {
                    target_uid: invocation.plan.source_uid,
                    effect_type: sonettobuf::effect_type_enum::EffectType::Mustcrit as i32,
                    effect_num: 0,
                    effect_num1: None,
                    config_effect: 0,
                    reserve_id: execution.primary_target_uid,
                    reserve_str: None,
                },
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
    }
    if !resuming_phase
        && active_phase == Some(SkillPhase::Damage)
        && has_row_damage
        && execution.planned_crits.is_none()
    {
        plan::plan_crits(
            invocation,
            managers,
            pool,
            catalog,
            effect_skill_id,
            determinism,
            execution,
        );
    }
    Ok(ControlFlow::Continue((
        Prepared {
            effect,
            effect_skill_id,
            active_phase,
            resume_slot,
            resume_target,
            source_team,
            has_row_damage,
        },
        outputs,
    )))
}

fn selected_target_order(target: SkillTarget, configured_targets: &[i64]) -> Option<Vec<i64>> {
    let SkillTarget::Explicit(selected) = target else {
        return None;
    };
    let index = configured_targets
        .iter()
        .position(|target_uid| *target_uid == selected)?;
    if index == 0 {
        return None;
    }
    let mut targets = configured_targets.to_vec();
    targets.swap(0, index);
    Some(targets)
}
