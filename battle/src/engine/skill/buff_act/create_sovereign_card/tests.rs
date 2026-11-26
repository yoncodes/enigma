use crate::engine::runtime::drain::BattleAccess;
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
fn counted_resources_fail_before_mutation_and_stacks_are_spent_on_the_new_card() {
    crate::test_support::init_config();
    for (count, layer) in [(1, 0), (1, 3), (0, 6), (0, 3), (0, 0)] {
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
            BattleAccess::new(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
            ),
            BattleEvent::Kind(EventKind::RoundStartCard),
            ReactionLane::BuffActs,
            Some(&[10]),
        );
        if count > 0 {
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
                BattleAccess::new(
                    &mut managers,
                    &pool,
                    &catalog,
                    &mut RoundDeterminism::default(),
                    TargetContext::default(),
                ),
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
            let card = &managers.card.hand()[0];
            assert_eq!(card.skill_id, Some(31472131));
            assert!(!managers.card.can_use_card(card));
            let spent = layer.min(4);
            let levels = match spent {
                4 => r#"{"1":2,"2":2,"3":2,"4":2}"#,
                3 => r#"{"1":2,"2":2,"3":1,"4":1}"#,
                _ => r#"{"1":0,"2":0,"3":0,"4":0}"#,
            };
            assert_eq!(
                card.card_dataes[0].value,
                Some(format!(r#"{{"lock":true,"strengthen":{levels}}}"#))
            );
            let left = managers
                .buff
                .active_for(10)
                .find(|buff| buff.buff_id == Some(31470001))
                .map(|buff| buff.layer.unwrap_or_default());
            assert_eq!(
                left.filter(|layer| *layer > 0),
                (layer > spent).then_some(layer - spent)
            );
        }
    }
}
