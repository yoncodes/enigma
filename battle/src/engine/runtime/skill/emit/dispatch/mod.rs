mod phases;
mod setup;
mod slots;

use std::ops::ControlFlow;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
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
        target::{
            BehaviorTargetOrder, TargetPool, TargetRequest, TargetResolver,
            request::SOURCE_TARGET_CODE,
        },
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
use super::{
    effect_started as effect_started_op, effects_prepared as effects_prepared_op,
    phase_completed as phase_completed_op,
};

const LOG_TARGET: &str = module_path!();

struct EmissionContext<'a> {
    managers: &'a BattleManagers,
    pool: &'a TargetPool,
    catalog: &'a SkillEffectCatalog,
    trigger: &'a SkillOpTrigger,
}

struct Prepared<'a> {
    effect: &'a crate::engine::skill::effect::slot::ParsedSkillEffect,
    effect_skill_id: i32,
    active_phase: Option<SkillPhase>,
    resume_slot: usize,
    resume_target: usize,
    source_team: i32,
    has_row_damage: bool,
}

struct SlotOutput {
    ops: Vec<SkillEmissionOp>,
    fired_rules: Vec<(usize, crate::engine::skill::rule::DefinitionKey)>,
}

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
    let context = EmissionContext {
        managers,
        pool,
        catalog,
        trigger,
    };
    let (prepared, outputs) =
        match setup::prepare(&mut invocation, &context, determinism, execution)? {
            ControlFlow::Continue(prepared) => prepared,
            ControlFlow::Break(suspended) => return Ok(suspended),
        };
    let emitted = match slots::emit(
        &invocation,
        &context,
        &prepared,
        determinism,
        execution,
        outputs,
    )? {
        ControlFlow::Continue(emitted) => emitted,
        ControlFlow::Break(suspended) => return Ok(suspended),
    };
    phases::finish(
        &invocation,
        &context,
        &prepared,
        determinism,
        execution,
        emitted,
    )
}

fn later_attack_modifier_runs_in_phase(
    slots: &[SkillEffectSlot],
    phase: SkillPhase,
    skill_id: i32,
) -> Result<bool, SkillOpError> {
    for slot in slots {
        if attack_modifier_reads_action_start(slot) && slot_runs_in_phase(slot, phase, skill_id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn attack_modifier_reads_action_start(slot: &SkillEffectSlot) -> bool {
    behavior::registry::find(&slot.behavior)
        .is_some_and(|definition| definition.attack_modifier_only)
        && slot.conditions.iter().any(|condition| {
            crate::engine::skill::condition::registry::find_key(
                condition.opcode,
                &condition.type_name,
            )
            .is_some_and(|definition| {
                definition.read_timing
                    == crate::engine::skill::condition::registry::ConditionReadTiming::ActionStart
            })
        })
}

fn skill_runs_in_phase(
    slots: &[SkillEffectSlot],
    phase: SkillPhase,
    skill_id: i32,
) -> Result<bool, SkillOpError> {
    for slot in slots {
        if slot_runs_in_phase(slot, phase, skill_id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn slot_runs_in_phase(
    slot: &SkillEffectSlot,
    phase: SkillPhase,
    skill_id: i32,
) -> Result<bool, SkillOpError> {
    let routed_phases = slot
        .active_phases()
        .map_err(|route| SkillOpError::UncompiledRoute { skill_id, route })?;
    if routed_phases.is_empty() {
        Ok(behavior::registry::find(&slot.behavior)
            .is_some_and(|definition| definition.phase == phase))
    } else {
        Ok(routed_phases.contains(&phase))
    }
}

pub(in crate::engine::runtime::skill) fn action_mode(
    mode: SkillExecutionMode,
    extra_kind: Option<crate::engine::skill::condition::extra::ExtraSkillKind>,
) -> SkillExecutionMode {
    // A riposte is an ally action too: "after any ally takes an action" reactions answer it.
    if mode == SkillExecutionMode::Nested
        && extra_kind.is_some_and(|kind| {
            kind.is_extra_action()
                || kind == crate::engine::skill::condition::extra::ExtraSkillKind::Riposte
        })
    {
        SkillExecutionMode::Active
    } else {
        mode
    }
}

fn emit_additional_damage_activation(
    invocation: &SkillInvocation,
    execution: &mut SkillExecution,
    activation: plan::AdditionalDamageActivation,
) -> Vec<SkillEmissionOp> {
    let feature = &activation.additional.feature;
    execution.activate_additional_damage(activation.additional.clone());
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
    activation
        .buff_act_ops
        .into_iter()
        .map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: frame_owner.clone(),
        })
        .chain(activation.skill_ops.into_iter().map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        }))
        .collect()
}

#[cfg(test)]
mod tests;
