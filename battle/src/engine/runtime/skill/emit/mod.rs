use crate::engine::{
    runtime::record::FrameOwner,
    skill::{
        action::{SkillActionEvent, SkillInvocation, SkillLifecycle, SkillPhase},
        behavior::registry::OutputOwner,
        condition::registry::ConsequencePolicy,
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
        target::TargetPool,
    },
};

use super::SkillExecution;

pub(in crate::engine::runtime) struct SkillEmission {
    pub(in crate::engine::runtime) ops: Vec<SkillEmissionOp>,
    pub(in crate::engine::runtime) fired_rules:
        Vec<(usize, crate::engine::skill::rule::DefinitionKey)>,
    pub(in crate::engine::runtime) continuation: Option<SkillInvocation>,
    pub(in crate::engine::runtime) target_uid: Option<i64>,
}

pub(in crate::engine::runtime) struct SkillEmissionOp {
    pub(in crate::engine::runtime) op: RuleOp,
    pub(in crate::engine::runtime) owner: OutputOwner,
    pub(in crate::engine::runtime) consequence: ConsequencePolicy,
    pub(in crate::engine::runtime) frame_owner: Option<FrameOwner>,
}

pub(super) fn phase_completed(
    invocation: &SkillInvocation,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    execution: &SkillExecution,
    phase: SkillPhase,
) -> SkillEmissionOp {
    SkillEmissionOp {
        op: RuleOp::SkillLifecycle(SkillLifecycle::PhaseCompleted(action_event(
            invocation, catalog, pool, execution, phase,
        ))),
        owner: OutputOwner::Skill,
        consequence: ConsequencePolicy::Default,
        frame_owner: None,
    }
}

pub(super) fn effect_started(
    invocation: &SkillInvocation,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    execution: &SkillExecution,
) -> SkillEmissionOp {
    SkillEmissionOp {
        op: RuleOp::Publish(
            crate::engine::event::payload::BattleEvent::SkillEffectStarted(action_event(
                invocation,
                catalog,
                pool,
                execution,
                SkillPhase::Immediate,
            )),
        ),
        owner: OutputOwner::Skill,
        consequence: ConsequencePolicy::Default,
        frame_owner: None,
    }
}

fn action_event(
    invocation: &SkillInvocation,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    execution: &SkillExecution,
    phase: SkillPhase,
) -> SkillActionEvent {
    SkillActionEvent {
        source_uid: invocation.plan.source_uid,
        skill_id: invocation.plan.skill_id,
        target_uid: execution
            .primary_target_uid
            .or_else(|| execution.configured_targets.as_ref()?.first().copied())
            .unwrap_or(execution.context.runtime_target_uid),
        target_uids: execution.affected_targets.clone(),
        attacked_target_uids: execution.attacked_targets.clone(),
        phase,
        skill_slot: pool.skill_slot(invocation.plan.source_uid, invocation.plan.skill_id),
        is_attack: catalog.is_attack(invocation.plan.skill_id),
        rank: crate::engine::entity::skill::skill_rank(invocation.plan.skill_id),
        skill_type: catalog.skill_type(invocation.plan.skill_id),
        effect_tag: catalog.effect_tag(invocation.plan.skill_id),
        assassinate: execution.context.active_skill_assassinate,
        ignore_riposte: execution.modifiers.ignore_riposte,
        damage_amount: execution.context.action_damage_amount,
        kill_count: execution.context.action_kill_count,
        crit_count: execution.context.action_crit_count,
        guard_break_count: execution.context.action_guard_break_count,
        additional_moxie: invocation.additional_moxie,
        extra_skill_kind: execution.context.extra_skill_kind,
        mode: invocation.mode,
        teammate_injury_count: execution.injured_allies.len() as i32,
        teammate_injury_count_not_reset: execution.injured_allies.len() as i32,
        team_injury_count_round: execution.team_injury_count_round,
        card_enchants: invocation.card_enchants.clone(),
        buff_additions: execution.buff_additions.clone(),
    }
}

mod dispatch;

pub(in crate::engine::runtime) use dispatch::emit_ops;
#[cfg(test)]
pub(super) use dispatch::{action_mode, skill_destination_already_emitted};
