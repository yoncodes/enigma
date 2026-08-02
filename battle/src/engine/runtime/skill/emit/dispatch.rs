use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillPhase, SkillTarget},
        behavior::{self, BehaviorOpContext, classify::BehaviorKind},
        condition::{
            ParsedConditionKind, conditions_fire_count, conditions_match,
            registry::ConsequencePolicy, resource::ResourceConditionContext,
            satisfied_card_enchants, satisfied_conditions,
        },
        effect::{SkillEffectCatalog, SkillEffectSlot},
        rule::{
            output::{BattleCommand, RuleOp},
            route::ConditionDriver,
        },
        target::{TargetPool, TargetRequest, TargetResolver},
    },
};

use super::super::{
    SkillExecution, SkillOpError, SkillOpTrigger,
    invoke::{
        active_skill_hit_targets, active_skill_targets, apply_event_context, resource_event,
        resource_fire_count,
    },
    plan,
};
use super::*;
use super::{effect_started as effect_started_op, phase_completed as phase_completed_op};

fn uses_action_targets(slot: &SkillEffectSlot, active_skill_target_condition: bool) -> bool {
    slot.target.code == 0
        || (slot.target_from_condition
            && (slot.condition_target.code == 0 || active_skill_target_condition))
}

pub(in crate::engine::runtime) fn emit_ops(
    mut invocation: SkillInvocation,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
    trigger: &SkillOpTrigger,
) -> Result<SkillEmission, SkillOpError> {
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
        .filter(|issue| {
            issue.reason
                != crate::engine::skill::effect::catalog::RuleIssueReason::UnsupportedBehavior
        })
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
    execution.context.damage_target_count_kind =
        crate::engine::skill::target::request::damage_target_count_kind(
            execution.context.logic_target,
        );
    execution.context.extra_skill_kind = invocation
        .extra_skill_kind
        .map(|kind| kind.id())
        .unwrap_or_else(|| catalog.extra_kind(invocation.plan.skill_id));
    if matches!(
        invocation.mode,
        crate::engine::skill::action::SkillExecutionMode::Active
            | crate::engine::skill::action::SkillExecutionMode::DirectBig
    ) {
        execution.context.active_skill_is_attack = catalog.is_attack(effect_skill_id);
        if matches!(trigger, SkillOpTrigger::Active) && invocation.card_index > 0 {
            execution.context.active_skill_slot =
                pool.skill_slot(invocation.plan.source_uid, invocation.plan.skill_id);
            execution.context.active_skill_rank =
                crate::engine::entity::skill::skill_rank(invocation.plan.skill_id);
            execution.context.active_skill_type = catalog.skill_type(effect_skill_id);
            execution.context.active_skill_effect_tag = catalog.effect_tag(effect_skill_id);
        }
        execution.context.additional_moxie = invocation.additional_moxie;
        execution.context.direct_skill_body = true;
    }
    if let SkillTarget::Explicit(uid) = invocation.target {
        execution.context.runtime_target_uid = uid;
        execution.primary_target_uid.get_or_insert(uid);
        execution.record_targets([uid]);
    }
    if let SkillOpTrigger::Event(event) = trigger {
        apply_event_context(&mut execution.context, event);
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
    let mut fired_rules = Vec::new();
    let active_phase = matches!(trigger, SkillOpTrigger::Active).then_some(
        invocation
            .phase
            .unwrap_or(crate::engine::skill::action::SkillPhase::Immediate),
    );
    if active_phase == Some(crate::engine::skill::action::SkillPhase::Immediate)
        && execution.configured_targets.is_none()
    {
        let request = TargetRequest {
            code: execution.context.logic_target,
            raw: Vec::new(),
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
        let configured_targets = if routes_configured_damage && !needs_action_targets {
            Vec::new()
        } else {
            TargetResolver::resolve_action_targets(
                &request,
                invocation.plan.skill_id,
                invocation.plan.source_uid,
                pool,
                determinism,
                Some(managers),
                execution.context,
            )
        };
        execution.configured_additional_targets = determinism
            .take_skill_target_choice(
                invocation.plan.skill_id,
                invocation.plan.source_uid,
                request.code,
            )
            .map(|choice| choice.additional_targets)
            .filter(|targets| !targets.is_empty());
        if let Some(&main_target) = configured_targets.first() {
            execution.context.runtime_target_uid = main_target;
            execution.primary_target_uid.get_or_insert(main_target);
        }
        execution.record_targets(configured_targets.iter().copied());
        execution.configured_targets = Some(configured_targets);
    }
    if active_phase == Some(SkillPhase::Immediate) {
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
        let effect_started_subscribers =
            crate::engine::skill::subscriber::for_compiled_owner_events(
                pool,
                managers,
                catalog,
                [crate::engine::event::kind::EventKind::SkillEffectStarted],
                &[invocation.plan.source_uid],
            )
            .map_err(SkillOpError::from)?;
        if !effect_started_subscribers.skills.is_empty()
            || !effect_started_subscribers.buff_acts.is_empty()
        {
            outputs.push(effect_started_op(&invocation, catalog, pool, execution));
        }
    }
    let has_row_damage = catalog.damage_rate(effect_skill_id) > 0
        && !effect
            .slots
            .iter()
            .any(|slot| slot.behavior.spec.kind == BehaviorKind::IgnoreSkillConfigDamageRate);
    if active_phase == Some(SkillPhase::Damage)
        && has_row_damage
        && execution.planned_crits.is_none()
    {
        plan::plan_crits(
            &invocation,
            managers,
            pool,
            catalog,
            effect_skill_id,
            determinism,
            execution,
        );
    }
    for (slot_index, slot) in effect.slots.iter().enumerate() {
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
        if let Some(phase) = active_phase {
            let routed_phases =
                slot.active_phases()
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?;
            if if routed_phases.is_empty() {
                definition.phase != phase
            } else {
                !routed_phases.contains(&phase)
            } {
                continue;
            }
        }
        if skill_destination_already_emitted(&outputs, definition, &slot.behavior) {
            continue;
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
                let matches = slot
                    .compiled_subscriptions()
                    .map_err(|route| SkillOpError::UncompiledRoute {
                        skill_id: invocation.plan.skill_id,
                        route,
                    })?
                    .into_iter()
                    .any(|key| {
                        key.definition == condition_key
                            && event
                                .subscription_kinds()
                                .any(|published| published == key.event)
                            && key.phase == invocation.phase
                    });
                if !matches {
                    continue;
                }
                (
                    satisfied_conditions(&slot.conditions, condition_key),
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
        if condition_key.is_some_and(|condition_key| {
            !managers.can_fire_rule(
                invocation.plan.source_uid,
                invocation.plan.skill_id,
                slot_index,
                condition_key,
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
        let per_target_conditions = slot.target_from_condition
            && crate::engine::skill::condition::registry::conditions_filter_behavior_targets(
                &conditions,
            );
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
                        matches!(condition.kind, ParsedConditionKind::Random { .. })
                    })
                {
                    context.condition_random_roll = Some(
                        determinism.condition_random_roll(invocation.plan.skill_id, random.opcode),
                    );
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
        let consequence = consequence_policy(slot, &invocation, trigger)?;
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
        let uses_action_targets = uses_action_targets(
            slot,
            condition_uses_active_skill_targets || condition_uses_hit_targets,
        );
        let event_targets = if condition_uses_hit_targets {
            selected_event.and_then(active_skill_hit_targets)
        } else if condition_uses_active_skill_targets {
            selected_event.and_then(active_skill_targets)
        } else {
            None
        };
        let mut targets = if let Some(targets) = event_targets {
            targets.to_vec()
        } else if active_phase.is_some()
            && has_row_damage
            && condition_uses_hit_targets
            && uses_action_targets
        {
            execution.attacked_targets.clone()
        } else if active_phase.is_some()
            && uses_action_targets
            && let Some(targets) = &execution.configured_targets
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
        if per_target_conditions {
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
        let outputs_before = outputs.len();
        for target_uid in targets {
            if !has_row_damage && target_uid != 0 {
                execution.primary_target_uid.get_or_insert(target_uid);
            }
            let (emissions, transfer_count) = match definition.fire_count_mode {
                behavior::registry::FireCountMode::Repeat => (fire_count, 1),
                behavior::registry::FireCountMode::Transfer => (1, fire_count),
            };
            for _ in 0..emissions {
                let behavior_ops = (definition.emit_ops)(
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
                )
                .ok_or(SkillOpError::MissingBehaviorOp {
                    skill_id: invocation.plan.skill_id,
                    key: definition.key,
                })?;
                outputs.extend(behavior_ops.into_iter().enumerate().map(|(index, op)| {
                    let owner = (definition.output_owner_for)(&slot.behavior, index)
                        .unwrap_or(definition.output_owner)
                        .resolve(
                            matches!(trigger, SkillOpTrigger::Event(_)),
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
        }
        if outputs.len() > outputs_before
            && let Some(condition_key) = condition_key
        {
            fired_rules.push((slot_index, condition_key));
        }
    }
    if active_phase == Some(SkillPhase::Immediate) {
        let mut phase_completed =
            phase_completed_op(&invocation, catalog, pool, execution, SkillPhase::Immediate);
        if let Some(cost) = execution.take_action_cost() {
            let RuleOp::SkillLifecycle(lifecycle) = phase_completed.op else {
                unreachable!("a completed phase emits a skill lifecycle")
            };
            phase_completed.op = RuleOp::BeginSkillAction { lifecycle, cost };
            outputs.insert(0, phase_completed);
        } else {
            outputs.push(phase_completed);
        }
    }
    if active_phase == Some(SkillPhase::Immediate) && has_row_damage {
        let activations = plan::additional_damage_activation(&invocation, managers, execution);
        for activation in activations {
            let feature = &activation.additional.feature;
            execution
                .activated_additional_damage
                .push(activation.additional.clone());
            execution
                .temporary_damage_buffs
                .extend(activation.temporary_buff);
            let frame_owner =
                crate::engine::skill::buff_act::feature_command_origin(feature).map(|origin| {
                    crate::engine::runtime::record::FrameOwner::BuffAct {
                        owner_uid: invocation.plan.source_uid,
                        source_uid: feature.source_uid,
                        buff_uid: feature.buff_uid,
                        buff_id: feature.buff_id,
                        key: origin.key,
                    }
                });
            outputs.extend(
                activation
                    .buff_act_ops
                    .into_iter()
                    .map(|op| SkillEmissionOp {
                        op,
                        owner: behavior::registry::OutputOwner::Skill,
                        consequence: ConsequencePolicy::Default,
                        frame_owner: frame_owner.clone(),
                    }),
            );
            outputs.extend(activation.skill_ops.into_iter().map(|op| SkillEmissionOp {
                op,
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            }));
        }
    }
    let mut has_after_damage = false;
    let mut has_after_hit = false;
    for slot in &effect.slots {
        let Some(definition) = behavior::registry::find(&slot.behavior) else {
            continue;
        };
        let routed_phases =
            slot.active_phases()
                .map_err(|route| SkillOpError::UncompiledRoute {
                    skill_id: invocation.plan.skill_id,
                    route,
                })?;
        let runs_in = |phase| {
            if routed_phases.is_empty() {
                definition.phase == phase
            } else {
                routed_phases.contains(&phase)
            }
        };
        has_after_damage |= runs_in(SkillPhase::AfterDamage);
        has_after_hit |= runs_in(SkillPhase::AfterHit);
    }
    has_after_damage |= execution.has_after_damage_ops();
    if matches!(
        active_phase,
        Some(SkillPhase::Immediate | SkillPhase::AdditionalDamage)
    ) {
        let subscribers = crate::engine::skill::subscriber::for_compiled_owner_events(
            pool,
            managers,
            catalog,
            [crate::engine::event::kind::EventKind::SkillAction],
            &[invocation.plan.source_uid],
        )
        .map_err(SkillOpError::from)?;
        has_after_damage |= subscribers
            .skills
            .into_iter()
            .any(|subscriber| subscriber.key.phase == Some(SkillPhase::AfterDamage))
            || subscribers
                .buff_acts
                .into_iter()
                .any(|subscriber| subscriber.key.phase == Some(SkillPhase::AfterDamage));
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::Damage) && has_row_damage {
        let damage = plan::damage_ops(
            &invocation,
            managers,
            pool,
            catalog,
            effect_skill_id,
            determinism,
            execution,
        );
        let damage_buff_act_frame_owner = damage.buff_act_frame_owner;
        execution.pending_additional_damage = damage.additional_damage;
        execution.set_after_damage_ops(damage.after_damage, damage_buff_act_frame_owner.clone());
        if damage.main_target.is_some() {
            execution.primary_target_uid = damage.main_target;
        }
        if let Some(key) = execution.take_team_injury_count_consumption() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Injury(
                    crate::engine::manager::injury::InjuryCommand::Reset {
                        origin: crate::engine::skill::rule::CommandOrigin {
                            domain: crate::engine::skill::rule::RuleDomain::Condition,
                            key,
                        },
                        team_type: source_team,
                    },
                )),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
        for feature in damage.avoided {
            let ops = crate::engine::skill::buff_act::dodge_spec_skill::trigger_rule_ops(&feature)
                .ok_or_else(|| SkillOpError::UnregisteredBuffAct {
                    opcode: feature.act_id().unwrap_or_default(),
                    type_name: feature.act_type.clone(),
                })?;
            outputs.extend(ops.into_iter().map(|op| SkillEmissionOp {
                op,
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            }));
        }
        outputs.extend(damage.before_damage.into_iter().map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: damage_buff_act_frame_owner.clone(),
        }));
        if !damage.damage.is_empty() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::HpBatch(damage.damage)),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) {
        let additional_damage_commands = execution.take_live_additional_damage(managers);
        if !additional_damage_commands.is_empty() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::HpBatch(additional_damage_commands)),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
        for additional in &execution.modifiers.additional_damage {
            let Some((feature, _)) = crate::engine::skill::buff_act::additional_damage::configured(
                additional.buff_id,
                invocation.plan.source_uid,
                invocation.plan.source_uid,
            ) else {
                continue;
            };
            let Some(origin) = crate::engine::skill::buff_act::feature_command_origin(&feature)
            else {
                continue;
            };
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                    origin,
                    target_uid: invocation.plan.source_uid,
                    selector: BuffRemoveSelector::ExactId(additional.buff_id),
                }))),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: Some(crate::engine::runtime::record::FrameOwner::BuffAct {
                    owner_uid: feature.owner_uid,
                    source_uid: feature.source_uid,
                    buff_uid: feature.buff_uid,
                    buff_id: feature.buff_id,
                    key: origin.key,
                }),
            });
        }
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AfterDamage) {
        let (after_damage_ops, frame_owner) = execution.take_after_damage_ops();
        outputs.extend(after_damage_ops.into_iter().map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: frame_owner.clone(),
        }));
    }
    let next_phase = match active_phase {
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_row_damage => {
            Some(crate::engine::skill::action::SkillPhase::Damage)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_after_damage => {
            Some(crate::engine::skill::action::SkillPhase::AfterDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_after_hit => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::Damage) => {
            Some(crate::engine::skill::action::SkillPhase::AdditionalDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) if has_after_damage => {
            Some(crate::engine::skill::action::SkillPhase::AfterDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::AfterDamage)
            if has_after_hit || has_row_damage =>
        {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::AfterDamage) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::HitPassives) => {
            Some(crate::engine::skill::action::SkillPhase::AfterHit)
        }
        _ => None,
    };
    let continuation = next_phase.map(|phase| {
        let mut continuation = invocation.clone();
        continuation.phase = Some(phase);
        continuation
    });
    let publishes_lifecycle =
        invocation.condition_key.is_none() && !matches!(trigger, SkillOpTrigger::Setup { .. });

    if continuation.is_none()
        && active_phase.is_some()
        && invocation.mode == crate::engine::skill::action::SkillExecutionMode::DirectBig
    {
        outputs.push(SkillEmissionOp {
            op: RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::DirectUltimateBodyCompleted {
                    source_uid: invocation.plan.source_uid,
                },
            ),
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        });
    }

    if publishes_lifecycle && active_phase == Some(SkillPhase::HitPassives) {
        outputs.push(phase_completed_op(
            &invocation,
            catalog,
            pool,
            execution,
            SkillPhase::HitPassives,
        ));
    } else if publishes_lifecycle && active_phase == Some(SkillPhase::AfterDamage) {
        outputs.push(phase_completed_op(
            &invocation,
            catalog,
            pool,
            execution,
            SkillPhase::AfterDamage,
        ));
        if continuation.is_none() {
            outputs.push(phase_completed_op(
                &invocation,
                catalog,
                pool,
                execution,
                SkillPhase::AfterHit,
            ));
        }
    } else if publishes_lifecycle && active_phase == Some(SkillPhase::AfterHit) {
        outputs.push(phase_completed_op(
            &invocation,
            catalog,
            pool,
            execution,
            SkillPhase::AfterHit,
        ));
    } else if publishes_lifecycle && active_phase.is_some() && continuation.is_none() {
        outputs.push(phase_completed_op(
            &invocation,
            catalog,
            pool,
            execution,
            SkillPhase::AfterDamage,
        ));
        outputs.push(phase_completed_op(
            &invocation,
            catalog,
            pool,
            execution,
            SkillPhase::AfterHit,
        ));
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AfterHit) {
        for (origin, buff_id) in std::mem::take(&mut execution.temporary_damage_buffs) {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                    origin,
                    target_uid: invocation.plan.source_uid,
                    selector: BuffRemoveSelector::ExactId(buff_id),
                }))),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
    }
    if continuation.is_none()
        && matches!(
            invocation.mode,
            crate::engine::skill::action::SkillExecutionMode::Active
                | crate::engine::skill::action::SkillExecutionMode::DirectBig
        )
    {
        outputs.push(SkillEmissionOp {
            op: RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::ActionCompleted(
                    crate::engine::skill::action::ActionEvent {
                        source_uid: invocation.plan.source_uid,
                        skill_id: invocation.plan.skill_id,
                        target_uid: execution.primary_target_uid.unwrap_or_default(),
                        target_uids: execution.affected_targets.clone(),
                        skill_slot: pool
                            .skill_slot(invocation.plan.source_uid, invocation.plan.skill_id),
                        is_attack: catalog.is_attack(effect_skill_id),
                        rank: crate::engine::entity::skill::skill_rank(invocation.plan.skill_id),
                        skill_type: catalog.skill_type(effect_skill_id),
                        effect_tag: catalog.effect_tag(effect_skill_id),
                        additional_moxie: invocation.additional_moxie,
                        extra_skill_kind: invocation
                            .extra_skill_kind
                            .map(|kind| kind.id())
                            .unwrap_or_default(),
                        mode: invocation.mode,
                        assassinate: execution.context.active_skill_assassinate,
                        damage_amount: execution.context.action_damage_amount,
                        kill_count: execution.context.action_kill_count,
                        crit_count: execution.context.action_crit_count,
                        teammate_injury_count: execution.injured_allies.len() as i32,
                        teammate_injury_count_not_reset: execution.injured_allies.len() as i32,
                        team_injury_count_round: execution.team_injury_count_round,
                        card_enchants: invocation.card_enchants.clone(),
                    },
                ),
            ),
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        });
    }
    Ok(SkillEmission {
        ops: outputs,
        fired_rules,
        continuation,
        target_uid: execution.primary_target_uid,
    })
}

pub(in crate::engine::runtime::skill) fn action_mode(
    mode: SkillExecutionMode,
    extra_kind: Option<crate::engine::skill::condition::extra::ExtraSkillKind>,
) -> SkillExecutionMode {
    if mode == SkillExecutionMode::Nested && extra_kind.is_some_and(|kind| kind.is_extra_action()) {
        SkillExecutionMode::Active
    } else {
        mode
    }
}

pub(in crate::engine::runtime::skill) fn skill_destination_already_emitted(
    outputs: &[SkillEmissionOp],
    definition: &crate::engine::skill::behavior::registry::BehaviorDefinition,
    behavior: &crate::engine::skill::effect::ParsedBehavior,
) -> bool {
    if definition.skill_destination_mode
        != crate::engine::skill::behavior::registry::SkillDestinationMode::Unique
    {
        return false;
    }
    let references = (definition.references)(behavior);
    let [skill_id] = references.skills.as_slice() else {
        return false;
    };
    outputs.iter().any(|output| {
        matches!(
            &output.op,
            RuleOp::Skill(invocation) if invocation.plan.skill_id == *skill_id
        )
    })
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
