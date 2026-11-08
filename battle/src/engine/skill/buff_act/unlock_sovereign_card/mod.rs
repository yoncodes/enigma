use super::{
    BuffActRuleError, BuffActRuleOp,
    registry::{BuffActKind, RuntimeContext},
};
use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::card::CardCommand,
    skill::{
        action::{SkillInvocation, SkillRequest},
        rule::{
            DefinitionKey,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockFailure {
    InvalidContext,
    InvalidCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockError {
    pub key: DefinitionKey,
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub event: Option<EventKind>,
    pub effect_time: i32,
    pub team_type: i32,
    pub linked_skill: Option<i32>,
    pub failure: UnlockFailure,
}

pub fn rule_ops(context: &mut RuntimeContext<'_>) -> Result<Vec<BuffActRuleOp>, BuffActRuleError> {
    let subscriber = context.subscriber;
    let error = |failure| {
        BuffActRuleError::UnlockSovereignCard(UnlockError {
            key: subscriber.key.definition,
            owner_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            event: context.event.map(BattleEvent::kind),
            effect_time: subscriber.effect_time,
            team_type: subscriber.team_type,
            linked_skill: subscriber.args.first().copied(),
            failure,
        })
    };
    if !super::subscriber_is_kind(subscriber, BuffActKind::SovereignCardUnlock)
        || subscriber.team_type != 1
        || context.managers.entity.team_type(subscriber.owner_uid) != Some(1)
        || context.event.map(BattleEvent::kind) != Some(EventKind::SovereignUnlock)
    {
        return Err(error(UnlockFailure::InvalidContext));
    }
    let [skill_id] = subscriber.args.as_slice() else {
        return Err(error(UnlockFailure::InvalidContext));
    };
    if !subscriber.owner_alive
        || !context
            .managers
            .card
            .can_unlock_sovereign(subscriber.owner_uid)
            .map_err(|_| error(UnlockFailure::InvalidCard))?
    {
        return Ok(Vec::new());
    }
    let origin =
        super::command_origin(subscriber).ok_or_else(|| error(UnlockFailure::InvalidContext))?;
    Ok(vec![
        BuffActRuleOp::subscriber_from_owner(RuleOp::Command(BattleCommand::Card(
            CardCommand::UnlockSovereign {
                origin,
                owner_uid: subscriber.owner_uid,
                team_type: subscriber.team_type,
            },
        ))),
        BuffActRuleOp::subscriber_from_owner(RuleOp::Skill(SkillInvocation::from(SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id: *skill_id,
        }))),
    ])
}

#[cfg(test)]
mod tests;
