use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::card::{CardCommand, CardInsertSovereign},
    skill::rule::{
        DefinitionKey,
        output::{BattleCommand, RuleOp},
    },
};

use super::{
    BuffActRuleError, BuffActRuleOp,
    registry::{BuffActKind, RuntimeContext},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationFailure {
    InvalidContext,
    MissingOwner,
    PositiveResource {
        resource_uid: i64,
        count: i32,
        layer: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateSovereignCardError {
    pub key: DefinitionKey,
    pub owner_uid: i64,
    pub source_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub team_type: i32,
    pub effect_time: i32,
    pub event: Option<EventKind>,
    pub args: Option<[i32; 4]>,
    pub failure: GenerationFailure,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill, position, resource, consume]
        if *skill > 0 && *position > 0 && *resource > 0 && *consume > 0)
}

pub fn scoped_rule_ops(
    context: &mut RuntimeContext<'_>,
) -> Result<Vec<BuffActRuleOp>, BuffActRuleError> {
    let subscriber = context.subscriber;
    let error = |failure| {
        BuffActRuleError::CreateSovereignCard(CreateSovereignCardError {
            key: subscriber.key.definition,
            owner_uid: subscriber.owner_uid,
            source_uid: subscriber.source_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            team_type: subscriber.team_type,
            effect_time: subscriber.effect_time,
            event: context.event.map(BattleEvent::kind),
            args: subscriber.args.as_slice().try_into().ok(),
            failure,
        })
    };
    if !super::subscriber_is_kind(subscriber, BuffActKind::SovereignCardGeneration)
        || subscriber.team_type != 1
        || context.managers.entity.team_type(subscriber.owner_uid) != Some(subscriber.team_type)
        || !matches!(
            context.event,
            Some(BattleEvent::Kind(EventKind::RoundStartCard))
        )
        || !supports(&subscriber.args)
    {
        return Err(error(GenerationFailure::InvalidContext));
    }
    if !subscriber.owner_alive
        || context
            .managers
            .card
            .has_sovereign_card(subscriber.owner_uid)
    {
        return Ok(Vec::new());
    }
    let [skill_id, max_position, resource_buff_id, _max_consume] = subscriber.args.as_slice()
    else {
        return Err(error(GenerationFailure::InvalidContext));
    };
    if let Some(resource) = context
        .managers
        .buff
        .active_for(subscriber.owner_uid)
        .find(|buff| {
            buff.buff_id == Some(*resource_buff_id)
                && (buff.count.unwrap_or_default() > 0 || buff.layer.unwrap_or_default() > 0)
        })
    {
        return Err(error(GenerationFailure::PositiveResource {
            resource_uid: resource.uid.unwrap_or_default(),
            count: resource.count.unwrap_or_default(),
            layer: resource.layer.unwrap_or_default(),
        }));
    }
    let hero_id = context
        .managers
        .entity
        .model_id(subscriber.owner_uid)
        .ok_or_else(|| error(GenerationFailure::MissingOwner))?;
    let origin = super::command_origin(subscriber)
        .ok_or_else(|| error(GenerationFailure::InvalidContext))?;
    Ok(vec![BuffActRuleOp::subscriber_from_owner(RuleOp::Command(
        BattleCommand::Card(CardCommand::InsertSovereign(CardInsertSovereign {
            origin,
            target_uid: subscriber.owner_uid,
            skill_id: *skill_id,
            hero_id,
            max_position: *max_position as usize,
            team_type: subscriber.team_type,
        })),
    ))])
}

#[cfg(test)]
mod tests;
