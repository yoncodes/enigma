use crate::engine::{
    manager::card::{CardCommand, CardHandLimitChange},
    round::modifier::RoundModifiers,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::AddActAndCardLimit, [action_points, card_limit])
                if *action_points != 0 || *card_limit != 0
        )
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [_, delta] = behavior.args.as_slice() else {
            return None;
        };
        if *delta == 0 {
            return Some(Vec::new());
        }
        let base = crate::engine::manager::card::start::hand_size_from_count(
            context
                .pool
                .attacker_main
                .iter()
                .filter(|entity| context.managers.hp.current(entity.uid) > 0)
                .count(),
            context.managers.fight_version(),
        );
        let current = crate::engine::mechanic::card::CardMechanic.normal_hand_limit(
            base,
            context.managers,
            context.pool,
        );
        let resulting_limit = i32::try_from(current)
            .unwrap_or(i32::MAX)
            .saturating_add(*delta);
        Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::ChangeHandLimit(CardHandLimitChange {
                origin: super::command_origin(behavior)?,
                target_uid: context.target_uid,
                delta: *delta,
                resulting_limit,
            }),
        ))])
    }

    fn collect_round_modifier(behavior: &ParsedBehavior) -> Option<RoundModifiers> {
        Self::supports(behavior).then(|| RoundModifiers {
            action_points: behavior.args[0],
        })
    }
}

#[cfg(test)]
mod tests;
