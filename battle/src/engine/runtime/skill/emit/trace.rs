use serde_json::{Value, json};

use crate::engine::{
    diagnostics::{self, TraceArea},
    manager::BattleManagers,
    skill::{
        action::SkillInvocation,
        effect::SkillEffectSlot,
        target::{TargetContext, TargetEntity, TargetPool},
    },
};

use super::super::SkillOpTrigger;

pub(super) fn targets(
    invocation: &SkillInvocation,
    slot: &SkillEffectSlot,
    trigger: &SkillOpTrigger,
    context: TargetContext,
    managers: &BattleManagers,
    pool: &TargetPool,
    selected: &[i64],
) {
    if diagnostics::enabled(TraceArea::Target) {
        eprintln!(
            "{}",
            record(invocation, slot, trigger, context, managers, pool, selected)
        );
    }
}

fn record(
    invocation: &SkillInvocation,
    slot: &SkillEffectSlot,
    trigger: &SkillOpTrigger,
    context: TargetContext,
    managers: &BattleManagers,
    pool: &TargetPool,
    selected: &[i64],
) -> Value {
    json!({
        "trace": "skill-target",
        "skill": invocation.plan.skill_id,
        "effect": slot.behavior.effect_id,
        "slot": slot.behavior.slot,
        "source": invocation.plan.source_uid,
        "behavior": {
            "opcode": slot.behavior.spec.key.opcode,
            "type": slot.behavior.spec.key.type_name,
            "args": slot.behavior.raw_args,
        },
        "target": {"code": slot.target.code, "raw": slot.target.raw},
        "phase": invocation.phase.map(|phase| format!("{phase:?}")),
        "mode": format!("{:?}", invocation.mode),
        "trigger": format!("{trigger:?}"),
        "context": {
            "battle": context.battle_id,
            "round": context.current_round,
            "activeSkill": context.active_skill_id,
            "activeSource": context.active_skill_source_uid,
            "runtimeTarget": context.runtime_target_uid,
            "eventSource": context.event_source_uid,
        },
        "selected": selected,
        "allies": health(pool.allies(invocation.plan.source_uid), managers),
        "enemies": health(pool.enemies(invocation.plan.source_uid, false), managers),
    })
}

fn health(entities: &[TargetEntity], managers: &BattleManagers) -> Vec<Value> {
    entities.iter().map(|entity| json!({
        "uid": entity.uid,
        "position": entity.position,
        "pool": {"current": entity.current_hp, "max": entity.max_hp},
        "manager": {"current": managers.hp.current(entity.uid), "max": managers.hp.max(entity.uid)},
    })).collect()
}

#[cfg(test)]
mod tests;
