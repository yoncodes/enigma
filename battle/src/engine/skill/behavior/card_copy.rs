use crate::engine::{
    event::payload::BattleEvent,
    manager::card::{CardCommand, CardQueueUse},
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

const BASIC_SKILL_SLOTS: [i32; 2] = [1, 2];

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports_arguments(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let BattleEvent::ActionQueueCommitted {
            team,
            cards,
            action_count,
            ..
        } = context.event?
        else {
            return Some(Vec::new());
        };
        if *team != context.source_team {
            return Some(Vec::new());
        }
        let (tags, count) = arguments(behavior)?;
        let catalog = context.managers.catalog();
        let owner = context.pool.entity(context.source_uid)?;
        let Some(skill_id) = cards
            .iter()
            .filter(|card| card.uid == Some(context.source_uid))
            .filter_map(|card| card.skill_id)
            .find(|skill_id| {
                !crate::engine::mechanic::card::CardMechanic.is_ultimate_skill(
                    context.managers,
                    *skill_id,
                    owner,
                ) && tags.contains(&catalog.skill_effect_tag(*skill_id))
            })
        else {
            return Some(Vec::new());
        };
        let origin = super::command_origin(behavior)?;
        let first_index = action_count
            .checked_add(context.managers.card.queued_use_cards().len())?
            .checked_add(1)?;
        (0..count)
            .map(|offset| {
                let card_index = i32::try_from(first_index.checked_add(offset)?).ok()?;
                let card = crate::engine::manager::card::pool::card_for_target(owner, skill_id)?;
                let mut action: SkillInvocation = SkillRequest {
                    source_uid: context.source_uid,
                    skill_id,
                }
                .into();
                action.card_index = card_index;
                action.mode = SkillExecutionMode::Active;
                Some(RuleOp::Command(BattleCommand::Card(
                    CardCommand::QueueUseCard(CardQueueUse {
                        origin,
                        card_index,
                        card,
                        team_type: *team,
                        source_skill_id: skill_id,
                        reserve_id: origin.key.opcode,
                        action: Some(action),
                    }),
                )))
            })
            .collect()
    }
}

pub fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    arguments(behavior).is_some()
}

fn arguments(behavior: &ParsedBehavior) -> Option<(Vec<i32>, usize)> {
    if behavior.arg_list(0)? != BASIC_SKILL_SLOTS {
        return None;
    }
    let tags = behavior.arg_list(1).filter(|values| !values.is_empty())?;
    // Only the copy counts a reached description states; 109350205 carries 9 for "1 duplicate".
    let count = match behavior.arg(2)? {
        1 => 1,
        2 => 2,
        _ => return None,
    };
    Some((tags, count))
}

#[cfg(test)]
mod tests;
