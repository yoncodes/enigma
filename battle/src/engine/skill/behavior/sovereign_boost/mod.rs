use crate::engine::{
    event::payload::BattleEvent,
    manager::card::{CardCommand, CardInitialSovereignBoost},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [8])
            && matches!(behavior.raw_args.as_slice(), [distance] if distance == "8")
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if !Self::supports(behavior) {
            return None;
        }
        let Some(BattleEvent::SovereignCardCreated(created)) = context.event else {
            return None;
        };
        if context.source_uid == 0
            || created.owner_uid != context.source_uid
            || context.target_uid != context.source_uid
            || context.source_team != 1
            || created.team_type != context.source_team
        {
            return None;
        }
        Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::InitialSovereignBoost(CardInitialSovereignBoost {
                origin: super::command_origin(behavior)?,
                owner_uid: context.source_uid,
                index: created.index,
                distance: usize::try_from(behavior.args[0]).ok()?,
                team_type: created.team_type,
            }),
        ))])
    }
}

#[cfg(test)]
mod tests;
