use crate::engine::{
    event::kind::EventKind,
    manager::hp::HpCommand,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        buff_act::{self, registry::BuffActKind},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
        subscriber,
    },
};

pub(super) struct Handler;

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [rounds @ (1 | 2), 1, rate] if *rounds > 0 && *rate > 0
    ) && behavior.raw_args.len() <= 3
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind != BehaviorKind::SettleDotAndCostDotDuration || !supports(behavior) {
            return None;
        }
        let [rounds, _, rate] = behavior.args.as_slice() else {
            return None;
        };
        let origin = super::command_origin(behavior)?;
        let mut ops = Vec::new();
        let dots = subscriber::active_buffs_for_owners(
            context.managers,
            EventKind::RoundEnd,
            &[context.target_uid],
        )
        .into_iter()
        .filter(|dot| buff_act::subscriber_kind(dot) == Some(BuffActKind::Poison))
        .collect::<Vec<_>>();
        for _ in 0..*rounds {
            for dot in &dots {
                let Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(mut loss)))) =
                    buff_act::damage_over_time::detonation_rule_op(
                        context.managers,
                        context.pool,
                        context.determinism,
                        dot,
                        false,
                    )
                else {
                    continue;
                };
                loss.amount = loss.amount.saturating_mul(*rate) / 1000;
                if loss.amount <= 0 {
                    continue;
                }
                loss.origin = origin;
                loss.config_effect = behavior.config_effect;
                if let Some(hurt) = &mut loss.hurt {
                    hurt.display_amount = Some(loss.amount);
                }
                ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))));
            }
        }
        Some(ops)
    }
}

#[cfg(test)]
mod tests;
