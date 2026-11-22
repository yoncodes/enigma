pub mod action_point;
pub mod additional_damage;
pub mod attr_fix_by_burn_layer;
pub mod attr_fix_by_lost_hp;
pub mod buff;
pub mod card;
pub mod card_copy;
pub mod card_limit;
pub mod career;
pub mod classify;
pub mod contract;
pub mod crystal_card;
pub mod damage_target;
pub mod detonate;
pub mod electric;
pub mod fake_hp;
pub mod fight_progress;
pub mod focus_all_entity_buff;
pub mod foundation_counter;
pub mod gauge;
pub mod general;
pub mod heal_or_buff;
pub mod hedone;
pub mod hedone_card;
pub mod hedone_device_power;
pub mod injury_bank;
pub mod kill;
pub mod magic_circle;
pub mod monster_change;
pub mod nuo_di_ka;
pub mod periodic;
pub mod poison;
pub mod position;
pub mod power_queue_rank;
pub mod precast;
pub mod prepare_skill;
pub mod rate;
pub mod registry;
pub mod resource;
pub mod scene;
pub mod shell;
pub mod shield;
pub mod skill_modifier;
pub mod sovereign_boost;
pub mod sovereign_enhancement;
pub mod special_count;
pub mod strengthen;
pub mod summon;
pub mod synchronization;
pub mod toughness;
pub mod ultimate_kind;
pub mod use_skill;

use crate::engine::{
    event::payload::BattleEvent,
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::output::RuleOp,
        target::{TargetContext, TargetPool},
    },
};

pub struct BehaviorOpContext<'a> {
    pub source_uid: i64,
    pub source_team: i32,
    pub target_uid: i64,
    pub active_skill_id: i32,
    pub transfer_count: i32,
    pub event: Option<&'a BattleEvent>,
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub determinism: &'a mut RoundDeterminism,
    pub modifiers: &'a mut SkillModifiers,
    pub target: &'a mut TargetContext,
}

pub struct AttackModifierContext<'a> {
    pub operation: BehaviorOpContext<'a>,
    pub owner_skill_id: i32,
    pub conditions: &'a [crate::engine::skill::condition::ParsedCondition],
}

pub fn rule_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
    let definition = registry::find(behavior)?;
    if definition
        .supports
        .is_some_and(|supports| !supports(behavior))
    {
        return None;
    }
    (definition.emit_ops)(context, behavior)
}

pub fn is_supported(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior).is_some_and(|definition| {
        definition
            .supports
            .is_none_or(|supports| supports(behavior))
    })
}

pub fn is_slot_supported(slot: &SkillEffectSlot) -> bool {
    let Some(definition) = registry::find(&slot.behavior) else {
        return false;
    };
    is_supported(&slot.behavior)
        && (definition.supports_slot)(slot)
        && definition.required_event.is_none_or(|required| {
            slot.compiled_subscriptions()
                .is_ok_and(|subscriptions| subscriptions.iter().any(|key| key.event == required))
        })
}

pub fn has_destination(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior).is_some_and(|definition| definition.destination)
}

pub fn runs_after_row_damage(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior)
        .is_some_and(|definition| definition.phase == registry::BehaviorPhase::AfterDamage)
}

pub fn routes_configured_damage(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior)
        .is_some_and(|definition| definition.kind == classify::BehaviorKind::ConfiguredDamageTarget)
}

pub fn command_origin(
    behavior: &ParsedBehavior,
) -> Option<crate::engine::skill::rule::CommandOrigin> {
    registry::find(behavior).map(|definition| crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: definition.key,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod op_tests;
