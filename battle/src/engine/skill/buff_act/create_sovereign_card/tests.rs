use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::{
        determinism::RoundDeterminism,
        drain::{self, DrainError, ReactionLane},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, RuleDomain},
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn positive_generation_resources_fail_before_mutation_and_zero_resources_generate() {
    crate::test_support::init_config();
    for (count, layer) in [(1, 0), (0, 1), (0, 0)] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3147),
                    team_type: Some(1),
                    current_hp: Some(1000),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31472006),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31470001),
                            from_uid: Some(10),
                            count: Some(count),
                            layer: Some(layer),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31472006]);
        let before_cards = managers.card.clone();
        let before_buffs = managers.buff.active_for(10).cloned().collect::<Vec<_>>();
        let result = drain::run_group_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            BattleEvent::Kind(EventKind::RoundStartCard),
            ReactionLane::BuffActs,
            Some(&[10]),
        );
        if count > 0 || layer > 0 {
            let Err(DrainError::BuffActRule(BuffActRuleError::CreateSovereignCard(error))) = result
            else {
                panic!("expected an explicit generation failure");
            };
            assert_eq!(error.key, DefinitionKey::new(1133, "UnnamedAddSpCard"));
            assert_eq!(
                (
                    error.owner_uid,
                    error.source_uid,
                    error.buff_uid,
                    error.buff_id
                ),
                (10, 10, 20, 31472006)
            );
            assert_eq!(
                (error.effect_time, error.event),
                (105, Some(EventKind::RoundStartCard))
            );
            assert_eq!(error.args, Some([31472131, 9, 31470001, 4]));
            assert!(matches!(
                error.failure,
                GenerationFailure::PositiveResource {
                    resource_uid: 21,
                    ..
                }
            ));
            assert_eq!(managers.card, before_cards);
            assert_eq!(
                managers.buff.active_for(10).cloned().collect::<Vec<_>>(),
                before_buffs
            );
            managers
                .execute_card(CardCommand::InsertSovereign(CardInsertSovereign {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: error.key,
                    },
                    target_uid: 10,
                    skill_id: 31472131,
                    hero_id: 3147,
                    max_position: 9,
                    team_type: 1,
                }))
                .unwrap();
            let held_cards = managers.card.clone();
            drain::run_group_event(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
                BattleEvent::Kind(EventKind::RoundStartCard),
                ReactionLane::BuffActs,
                Some(&[10]),
            )
            .unwrap();
            assert_eq!(managers.card, held_cards);
            assert_eq!(
                managers.buff.active_for(10).cloned().collect::<Vec<_>>(),
                before_buffs
            );
        } else {
            result.unwrap();
            assert_eq!(managers.card.hand().len(), 1);
            assert_eq!(managers.card.hand()[0].skill_id, Some(31472131));
            assert!(!managers.card.can_use_card(&managers.card.hand()[0]));
        }
    }
}
