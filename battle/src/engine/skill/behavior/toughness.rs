use crate::engine::{
    manager::toughness::{ToughnessCommand, ToughnessRecover},
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
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::Toughness(
                ToughnessCommand::Recover(ToughnessRecover {
                    target_uid: context.target_uid,
                    config_effect: behavior.config_effect,
                }),
            ))]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_requires_the_exact_empty_shape() {
        let behavior = ParsedBehavior::new(60_287, "ToughnessRecover", vec![]);
        assert!(Handler::supports(&behavior));

        let malformed = ParsedBehavior::new(60_287, "ToughnessRecover", vec![1]);
        assert!(!Handler::supports(&malformed));
    }
}
