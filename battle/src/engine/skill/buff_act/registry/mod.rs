#[macro_use]
mod schema;

mod definitions;
mod feature;
mod kind;
mod model;

use crate::engine::{
    event::{
        kind::EventKind,
        subscription::{PublicationPhase, ReactionTiming},
    },
    skill::rule::{DefinitionKey, SetupStage, output::RuleOp},
};

pub use feature::{ParsedBuffAct, destination_for_feature, resolve_feature};
use feature::{empty_references, references_for_feature};
pub use kind::BuffActKind;
pub use model::*;

#[cfg(test)]
use feature::{BuffActParseError, parse_integer_projection};

pub fn definitions() -> impl Iterator<Item = &'static BuffActDefinition> {
    definitions::DEFINITIONS.iter()
}

pub fn transaction_definitions(
    event: EventKind,
) -> impl Iterator<Item = &'static BuffActDefinition> {
    definitions().filter(move |definition| definition.transaction.events.contains(&event))
}

pub(crate) fn expiry_publication(
    feature: &[i32],
    act_type: &str,
    stage: i32,
    fight_version: i32,
) -> Option<PublicationPhase> {
    use crate::engine::fight::versions::{RoundStartSetupLayout, round_start_setup_layout};
    if round_start_setup_layout(fight_version) != Some(RoundStartSetupLayout::Version7) {
        return None;
    }
    let [opcode, attribute, ..] = feature else {
        return None;
    };
    find(*opcode, act_type)?
        .transaction
        .expiry_publications
        .iter()
        .find_map(|(take_stage, attr, phase)| {
            (*take_stage == stage && attr.id() == *attribute).then_some(*phase)
        })
}

/// Exact buff-act support and execution gateway.
/// `effectTime` selects an event lane but never substitutes for this key.
pub fn find(opcode: i32, type_name: &str) -> Option<&'static BuffActDefinition> {
    definitions().find(|definition| definition.key.matches(opcode, type_name))
}

pub fn kind(opcode: i32, type_name: &str) -> Option<BuffActKind> {
    find(opcode, type_name).map(|definition| definition.kind)
}

pub fn reserves_trigger_child_uid(key: DefinitionKey) -> bool {
    find(key.opcode, key.type_name)
        .is_some_and(|definition| definition.runtime.reserves_trigger_child_uid)
}

pub fn owns_duration(opcode: i32, type_name: &str) -> bool {
    find(opcode, type_name).is_some_and(|definition| definition.runtime.owns_duration)
}

pub fn runtime_marker(key: DefinitionKey) -> Option<RuntimeMarker> {
    find(key.opcode, key.type_name)?.runtime.marker
}

pub fn runtime_event(act_id: i32, act_type: &str, effect_time: i32) -> Option<EventKind> {
    if let Some(definition) = find(act_id, act_type) {
        if !definition.runtime.effect_time_subscription {
            return None;
        }
        if let Some(event) = definition.runtime.event_override {
            return Some(event);
        }
    }
    match super::effect_time::classify(effect_time) {
        super::effect_time::BuffActEvent::Runtime(event) => Some(event),
        _ => None,
    }
}

pub fn runtime_phase(
    act_id: i32,
    act_type: &str,
    effect_time: i32,
    event: EventKind,
) -> Option<crate::engine::skill::action::SkillPhase> {
    let definition = find(act_id, act_type)?;
    if definition.runtime.event_override == Some(event)
        && let Some(phase) = definition.runtime.phase_override
    {
        return Some(phase);
    }
    (event == EventKind::SkillAction)
        .then(|| super::effect_time::find(effect_time)?.duration_phase)
        .flatten()
}

pub fn subscribes_to_event(
    act_id: i32,
    act_type: &str,
    effect_time: i32,
    event: EventKind,
) -> bool {
    find(act_id, act_type).is_some_and(|definition| definition.runtime.events.contains(&event))
        || runtime_event(act_id, act_type, effect_time) == Some(event)
}

pub fn runtime_publication(act_id: i32, act_type: &str, event: EventKind) -> PublicationPhase {
    find(act_id, act_type)
        .map(|definition| {
            definition
                .runtime
                .publications
                .iter()
                .find_map(|(registered, phase)| (*registered == event).then_some(*phase))
                .unwrap_or(definition.runtime.publication)
        })
        .unwrap_or_default()
}

pub fn runtime_team_scope(act_id: i32, act_type: &str) -> RuntimeTeamScope {
    find(act_id, act_type)
        .map(|definition| definition.runtime.team_scope)
        .unwrap_or_default()
}

pub fn runtime_actor_scope(act_id: i32, act_type: &str) -> RuntimeActorScope {
    find(act_id, act_type)
        .map(|definition| definition.runtime.actor_scope)
        .unwrap_or_default()
}

pub fn reaction_timing(act_id: i32, act_type: &str) -> ReactionTiming {
    find(act_id, act_type)
        .map(|definition| definition.runtime.reaction_timing)
        .unwrap_or_default()
}

pub fn linked_rule_ops(
    owner_uid: i64,
    opcode: i32,
    type_name: &str,
    args: &[i32],
) -> Option<Vec<RuleOp>> {
    super::use_skill::linked_for(owner_uid, opcode, type_name, args)
        .map(|plan| vec![RuleOp::Skill(plan.into())])
}

pub fn destination(opcode: i32, type_name: &str, args: &[i32]) -> Option<BuffActDestination> {
    destination_with_raw(None, opcode, type_name, args, None)
}

pub fn destination_with_raw(
    game: Option<&config::GameDB>,
    opcode: i32,
    type_name: &str,
    args: &[i32],
    raw: Option<&str>,
) -> Option<BuffActDestination> {
    let definition = find(opcode, type_name)?;
    if !definition.validate_arguments(game, args, raw) {
        return None;
    }
    definition.destination().or_else(|| {
        linked_rule_ops(0, opcode, type_name, args)
            .is_some()
            .then_some(BuffActDestination::LinkedSkill)
    })
}

pub fn has_destination(opcode: i32, type_name: &str, args: &[i32]) -> bool {
    destination(opcode, type_name, args).is_some()
}

#[cfg(test)]
mod tests;
