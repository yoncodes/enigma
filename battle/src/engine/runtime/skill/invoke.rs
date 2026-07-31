use crate::engine::{
    event::payload::BattleEvent,
    runtime::determinism::RoundDeterminism,
    skill::{
        condition::{
            ParsedCondition, ParsedConditionKind, query,
            resource::{self, ResourceConditionContext, ResourceEvent},
        },
        rule::SetupStage,
        target::TargetContext,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum SkillOpTrigger {
    Active,
    Event(BattleEvent),
    Setup { stage: SetupStage, priority: i32 },
}

pub(super) fn resource_event(event: &BattleEvent) -> Option<ResourceEvent> {
    match event {
        BattleEvent::ExPointChanged(change) => Some(ResourceEvent::ExPoint {
            target_uid: change.target_uid,
            kind: change.kind.as_wire(),
            delta: change.applied_delta,
        }),
        BattleEvent::EurekaChanged(change) => Some(ResourceEvent::Eureka {
            target_uid: change.target_uid,
            eureka_id: change.power_id,
            applied_delta: change.applied_delta,
            overflow: change.overflow,
        }),
        BattleEvent::ConduitActivated(change) => Some(ResourceEvent::Conduit {
            target_uid: change.source_uid,
            power_id: change.power_id,
            spent: change.spent,
        }),
        _ => None,
    }
}

pub(super) fn active_skill_targets(event: &BattleEvent) -> Option<&[i64]> {
    match event {
        BattleEvent::SkillEffectStarted(action) | BattleEvent::SkillAction(action) => {
            Some(&action.target_uids)
        }
        BattleEvent::AllyAction(action) => Some(&action.target_uids),
        _ => None,
    }
}

pub(super) fn active_skill_hit_targets(event: &BattleEvent) -> Option<&[i64]> {
    match event {
        BattleEvent::SkillAction(action) => Some(&action.attacked_target_uids),
        _ => None,
    }
}

pub(super) fn resource_fire_count(
    conditions: &[ParsedCondition],
    skill_id: i32,
    context: ResourceConditionContext<'_>,
    determinism: &mut RoundDeterminism,
) -> Option<i32> {
    if !resource::has_event_condition(conditions) {
        return None;
    }
    let count = resource::event_conditions_count(conditions, context);
    if count <= 0 {
        return Some(0);
    }
    let Some(random) = query::find(conditions, &|condition| {
        matches!(condition.kind, ParsedConditionKind::Random { .. })
    }) else {
        return Some(count);
    };
    Some(resource::event_conditions_count(
        conditions,
        ResourceConditionContext {
            random_roll: Some(determinism.condition_random_roll(skill_id, random.opcode)),
            ..context
        },
    ))
}

pub(super) fn apply_event_context(context: &mut TargetContext, event: &BattleEvent) {
    match event {
        BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change) => {
            context.runtime_target_uid = change.target_uid;
            context.added_buff_id = change.buff_id;
            context.added_buff_amount = (change.after_amount - change.before_amount).max(0);
            context.added_buff_target_uid = change.target_uid;
        }
        BattleEvent::BuffRemoved(change) => {
            context.runtime_target_uid = change.target_uid;
            context.removed_buff_id = change.buff_id;
            context.removed_buff_target_uid = change.target_uid;
        }
        BattleEvent::BuffFeatureTriggered(trigger) => {
            context.runtime_target_uid = trigger.target_uid;
            context.triggered_buff_act_id = trigger.act_id;
            context.triggered_buff_uid = trigger.buff_uid;
        }
        BattleEvent::HpLost { target_uid, .. } | BattleEvent::HpHealed { target_uid, .. } => {
            context.runtime_target_uid = *target_uid
        }
        BattleEvent::ToughnessBroken { target_uid, .. } => {
            context.runtime_target_uid = *target_uid;
            context.toughness_broken_uid = *target_uid;
        }
        BattleEvent::Hit(hit) => {
            context.runtime_target_uid = hit.target_uid;
            context.hit_source_uid = hit.source_uid;
            context.hit_target_uid = hit.target_uid;
            context.hit_damage_from = Some(hit.damage_from);
            context.active_skill_id = hit.skill_id;
            context.active_skill_source_uid = hit.source_uid;
        }
        BattleEvent::EntityDied(death) => context.runtime_target_uid = death.target_uid,
        BattleEvent::EntityEntered { target_uid }
        | BattleEvent::EntityTransformed { target_uid } => context.runtime_target_uid = *target_uid,
        BattleEvent::ExPointChanged(change) | BattleEvent::ExPointOverflow(change) => {
            context.runtime_target_uid = change.target_uid;
            context.ex_point_changed_uid = change.target_uid;
            context.ex_point_delta = change.applied_delta;
            context.additional_moxie = change.applied_delta.max(0);
        }
        BattleEvent::EurekaChanged(change) => {
            context.runtime_target_uid = change.target_uid;
            if change.applied_delta < 0 {
                context.lost_power_id = change.power_id;
                context.lost_power_amount = -change.applied_delta;
            }
        }
        BattleEvent::ConduitActivated(change) => {
            context.runtime_target_uid = change.source_uid;
        }
        BattleEvent::GaugeChanged(change) => match change.key.kind {
            crate::engine::manager::gauge::GaugeKind::Bloodtithe => {
                context.blood_pool_value = change.after;
                context.blood_pool_max = change.after_max.unwrap_or_default();
            }
            crate::engine::manager::gauge::GaugeKind::LingeringGlow => {
                if change.kind != crate::engine::manager::gauge::GaugeChangeKind::Accumulated {
                    context.heat_scale_value = change.after;
                    context.heat_scale_raw_value = change.after.saturating_mul(1000);
                }
            }
            _ => {}
        },
        BattleEvent::FieldChanged(change) => {
            context.magic_circle_id = change.field_id;
            if matches!(
                change.kind,
                crate::engine::manager::field::FieldChangeKind::Deployed
                    | crate::engine::manager::field::FieldChangeKind::Level
            ) {
                context.added_magic_circle_id = change.field_id;
            }
            if change.kind == crate::engine::manager::field::FieldChangeKind::Removed {
                context.removed_magic_circle_id = change.field_id;
            }
        }
        BattleEvent::ShellChanged(change) => {
            context.runtime_target_uid = change.target_uid;
            context.shell_change_amount = change.amount;
            context.shell_deployed_buff_id = change.deployed_buff_id;
        }
        BattleEvent::ImpromptuResolved {
            critical_action_count,
            ..
        } => {
            context.critical_action_count = *critical_action_count;
        }
        BattleEvent::EnterFight
        | BattleEvent::BattleTerminalCommitted { .. }
        | BattleEvent::RoundStart
        | BattleEvent::ActionQueueCommitted { .. }
        | BattleEvent::PlayerActionsResolved { .. }
        | BattleEvent::BuffsSettled(_)
        | BattleEvent::CardChanged(_)
        | BattleEvent::SummonChanged(_)
        | BattleEvent::BloodtitheChanged { .. }
        | BattleEvent::Kind(_) => {}
        BattleEvent::SkillEffectStarted(action) | BattleEvent::SkillAction(action) => {
            context.runtime_target_uid = action.target_uid;
            context.active_skill_id = action.skill_id;
            context.active_skill_source_uid = action.source_uid;
            context.active_skill_slot = action.skill_slot;
            context.active_skill_is_attack = action.is_attack;
            context.active_skill_rank = action.rank;
            context.active_skill_type = action.skill_type;
            context.active_skill_effect_tag = action.effect_tag;
            context.active_skill_assassinate = action.assassinate;
            context.action_damage_amount = action.damage_amount;
            context.action_dealt_damage = action.damage_amount > 0;
            context.action_kill_count = action.kill_count;
            context.action_crit_count = action.crit_count;
            context.action_guard_break_count = action.guard_break_count;
            context.additional_moxie = action.additional_moxie;
            context.extra_skill_kind = action.extra_skill_kind;
            context.teammate_injury_count = action.teammate_injury_count;
            context.teammate_injury_count_not_reset = action.teammate_injury_count_not_reset;
            context.team_injury_count_round = action.team_injury_count_round;
            context.direct_skill_body = true;
        }
        BattleEvent::AllyAction(action) => {
            context.runtime_target_uid = action.target_uid;
            context.active_skill_id = action.skill_id;
            context.active_skill_source_uid = action.source_uid;
            context.active_skill_slot = action.skill_slot;
            context.active_skill_is_attack = action.is_attack;
            context.active_skill_rank = action.rank;
            context.active_skill_type = action.skill_type;
            context.active_skill_effect_tag = action.effect_tag;
            context.additional_moxie = action.additional_moxie;
            context.extra_skill_kind = action.extra_skill_kind;
            context.action_damage_amount = action.damage_amount;
            context.action_dealt_damage = action.damage_amount > 0;
            context.action_kill_count = action.kill_count;
            context.action_crit_count = action.crit_count;
            context.active_skill_assassinate = action.assassinate;
            context.teammate_injury_count = action.teammate_injury_count;
            context.teammate_injury_count_not_reset = action.teammate_injury_count_not_reset;
            context.team_injury_count_round = action.team_injury_count_round;
            context.direct_skill_body = true;
        }
    }
}
