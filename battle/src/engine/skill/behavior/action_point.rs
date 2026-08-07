use crate::engine::{
    round::modifier::RoundModifiers,
    skill::{
        behavior::{classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
    },
};

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::AddAct | BehaviorKind::AddActHero, [amount]) if *amount != 0
        )
    }

    fn collect_round_modifier(behavior: &ParsedBehavior) -> Option<RoundModifiers> {
        Self::supports(behavior).then(|| RoundModifiers {
            action_points: behavior.args[0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_act_keeps_the_signed_configured_delta() {
        let add = ParsedBehavior::new(40003, "AddAct", vec![1]);
        let remove = ParsedBehavior::new(40003, "AddAct", vec![-1]);
        let hero_add = ParsedBehavior::new(50006, "AddActHero", vec![1]);

        assert_eq!(
            Handler::collect_round_modifier(&add),
            Some(RoundModifiers { action_points: 1 })
        );
        assert_eq!(
            Handler::collect_round_modifier(&remove),
            Some(RoundModifiers { action_points: -1 })
        );
        assert_eq!(
            Handler::collect_round_modifier(&hero_add),
            Some(RoundModifiers { action_points: 1 })
        );
    }
}
