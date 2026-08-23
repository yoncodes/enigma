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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            behavior::classify::BehaviorSpec,
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn configured_card_limit_commits_the_absolute_resulting_limit() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: (1..=4)
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        current_hp: Some(100),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(40007, "AddActAndCardLimit"),
            vec![0, 2],
            Vec::new(),
        );
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = Default::default();
        let mut target = TargetContext::default();

        assert!(matches!(
            Handler::emit_ops(
                BehaviorOpContext {
                    source_uid: -1,
                    source_team: 1,
                    target_uid: -1,
                    active_skill_id: 370001001,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            )
            .as_deref(),
            Some([RuleOp::Command(BattleCommand::Card(
                CardCommand::ChangeHandLimit(CardHandLimitChange {
                    target_uid: -1,
                    delta: 2,
                    resulting_limit: 10,
                    ..
                })
            ))])
        ));
    }
}
