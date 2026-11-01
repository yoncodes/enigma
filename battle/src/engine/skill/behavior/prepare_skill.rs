use super::{BehaviorOpContext, registry::BehaviorHandler};
use crate::engine::{
    manager::card::{CardCommand, CardPrepareAiSkill},
    skill::{
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [skill_id] if *skill_id > 0)
            && behavior.raw_args.len() <= 1
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if !Self::supports(behavior)
            || context.pool.team_type(context.target_uid) != Some(2)
            || context.managers.hp.current(context.target_uid) <= 0
        {
            return None;
        }
        Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::PrepareAiSkill(CardPrepareAiSkill {
                origin: super::command_origin(behavior)?,
                owner_uid: context.target_uid,
                skill_id: behavior.args[0],
            }),
        ))])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: if Self::supports(behavior) {
                behavior.args.clone()
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests;
