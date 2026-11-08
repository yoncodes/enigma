use super::{
    BuffActRuleError, BuffActRuleOp,
    registry::{BuffActKind, RuntimeContext},
};
use crate::engine::{
    event::payload::BattleEvent,
    manager::card::{CardCommand, ChargeTarget, SovereignEnhancement},
    skill::{
        action::{SkillExecutionMode, SkillPhase},
        effect::catalog::SkillEffectTag,
        rule::{
            DefinitionKey,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeFailure {
    InvalidContext,
    TargetDefinition,
    InvalidCard,
    QueuedCard { card_index: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChargeError {
    pub key: DefinitionKey,
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub source_uid: i64,
    pub skill_id: i32,
    pub card_index: i32,
    pub effect_time: i32,
    pub phase: Option<SkillPhase>,
    pub args: Option<[i32; 8]>,
    pub failure: ChargeFailure,
}

pub fn rule_ops(context: &mut RuntimeContext<'_>) -> Result<Vec<BuffActRuleOp>, BuffActRuleError> {
    let subscriber = context.subscriber;
    let action = context.event.and_then(|event| match event {
        BattleEvent::SkillAction(action) => Some(action),
        _ => None,
    });
    let error = |failure| {
        BuffActRuleError::ChargeSovereignCard(ChargeError {
            key: subscriber.key.definition,
            owner_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            source_uid: action.map(|action| action.source_uid).unwrap_or_default(),
            skill_id: action.map(|action| action.skill_id).unwrap_or_default(),
            card_index: action.map(|action| action.card_index).unwrap_or_default(),
            effect_time: subscriber.effect_time,
            phase: action.map(|action| action.phase),
            args: subscriber.args.as_slice().try_into().ok(),
            failure,
        })
    };
    if !super::subscriber_is_kind(subscriber, BuffActKind::SovereignCardCharge)
        || subscriber.team_type != 1
        || context.managers.entity.team_type(subscriber.owner_uid) != Some(1)
    {
        return Err(error(ChargeFailure::InvalidContext));
    }
    let action = action.ok_or_else(|| error(ChargeFailure::InvalidContext))?;
    if action.phase != SkillPhase::AfterHit {
        return Err(error(ChargeFailure::InvalidContext));
    }
    if !subscriber.owner_alive
        || action.card_index <= 0
        || !matches!(
            action.mode,
            SkillExecutionMode::Active
                | SkillExecutionMode::Synchronization
                | SkillExecutionMode::DeviceCard
        )
        || context.managers.entity.team_type(action.source_uid) != Some(subscriber.team_type)
    {
        return Ok(Vec::new());
    }
    let kind = match action.effect_tag {
        tag if [
            SkillEffectTag::RealityDamage as i32,
            SkillEffectTag::MentalDamage as i32,
            SkillEffectTag::Debuff as i32,
        ]
        .contains(&tag) =>
        {
            match context
                .managers
                .catalog()
                .skill_target_type(action.skill_id)
            {
                Some(1) => SovereignEnhancement::Single,
                Some(2) => SovereignEnhancement::Mass,
                Some(0) => return Ok(Vec::new()),
                _ => return Err(error(ChargeFailure::TargetDefinition)),
            }
        }
        tag if tag == SkillEffectTag::Buff as i32 || tag == SkillEffectTag::CounterSpell as i32 => {
            SovereignEnhancement::Support
        }
        tag if tag == SkillEffectTag::Heal as i32 || tag == SkillEffectTag::Channel as i32 => {
            SovereignEnhancement::Recovery
        }
        _ => return Ok(Vec::new()),
    };
    match context
        .managers
        .card
        .charge_target(subscriber.owner_uid, kind, action.card_index)
        .map_err(|_| error(ChargeFailure::InvalidCard))?
    {
        Some(ChargeTarget::Hand) => {}
        Some(ChargeTarget::Queued(card_index)) => {
            return Err(error(ChargeFailure::QueuedCard { card_index }));
        }
        None => return Ok(Vec::new()),
    }
    let origin =
        super::command_origin(subscriber).ok_or_else(|| error(ChargeFailure::InvalidContext))?;
    Ok(vec![BuffActRuleOp::subscriber_from_owner(RuleOp::Command(
        BattleCommand::Card(CardCommand::ChargeSovereign {
            origin,
            owner_uid: subscriber.owner_uid,
            kind,
            team_type: subscriber.team_type,
        }),
    ))])
}

#[cfg(test)]
mod tests;
