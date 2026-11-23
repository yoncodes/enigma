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
