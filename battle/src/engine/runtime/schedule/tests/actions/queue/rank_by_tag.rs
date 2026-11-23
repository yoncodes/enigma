use super::*;
use crate::engine::manager::card::CardRankResult;
use sonettobuf::{FightDeviceOper, effect_type_enum::EffectType};

#[test]
fn queue_commit_ranks_only_the_owners_matching_basic_incantations() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        team_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let lorentz = FightEntityInfo {
        skill_group1: vec![31390114, 31390115, 31390116],
        skill_group2: vec![31390121, 31390122, 31390123],
        passive_skill: vec![31390141],
        buffs: vec![BuffInfo {
            uid: Some(60),
            buff_id: Some(31390190),
            from_uid: Some(10),
            duration: Some(1),
            ..Default::default()
        }],
        ..entity(10, 3139)
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                lorentz,
                entity(20, 3134),
                entity(30, 3127),
                entity(40, 3145),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let selection = player::selection::Selection::from_operations(&[FightDeviceOper {
        uid: Some(40),
        index: Some(2),
    }]);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    for (cards, expected) in [
        (vec![(10, 31390121), (10, 31390132)], vec![]),
        (
            vec![(10, 31390132), (10, 31390121), (20, 31340121)],
            vec![(2, 31390122)],
        ),
        (
            vec![
                (10, 31390114),
                (10, 31390121),
                (30, 31270121),
                (20, 31340121),
                (10, 31390114),
            ],
            vec![(1, 31390115), (5, 31390115), (2, 31390123)],
        ),
    ] {
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_card(CardCommand::Setup(CardSetup {
                deck_num: cards.len() as i32,
                hand: cards
                    .into_iter()
                    .map(|(uid, skill_id)| CardInfo {
                        uid: Some(uid),
                        skill_id: Some(skill_id),
                        ..Default::default()
                    })
                    .collect(),
                draw_pile: Vec::new(),
            }))
            .unwrap();
        while !managers.card.hand().is_empty() {
            managers
                .execute_card(CardCommand::Play(CardPlay {
                    origin: CARD_PLAY_ORIGIN,
                    hand_index: 0,
                    target_uid: None,
                    chosen_skill_id: None,
                    choice: None,
                    recorded_skill: None,
                }))
                .unwrap();
        }
        let queue = queue_from_played(&managers, 1, 99_998);
        let committed = run_action_queue_committed(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            queue,
            &selection,
        )
        .unwrap();
        let actual: Vec<_> = committed
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Card(changes) => Some(&changes.rank_results),
                _ => None,
            })
            .flatten()
            .map(|result| {
                let CardRankResult::Changed(change) = result else {
                    panic!("unexpected rank settlement {result:?}");
                };
                assert_eq!(change.owner_uid, 10);
                assert!(change.rewritten);
                (change.card_index, change.card.skill_id.unwrap())
            })
            .collect();
        assert_eq!(actual, expected);
        let steps = crate::engine::packet::timeline::project(&committed.frames).unwrap();
        let mut following_steps = steps.iter().skip(1);
        if !expected.is_empty() {
            let rewrite = &following_steps.next().unwrap().act_effect[0];
            let ranks = &rewrite.fight_step.as_ref().unwrap().act_effect;
            assert_eq!(ranks.len(), expected.len());
            assert!(
                ranks
                    .iter()
                    .all(|effect| effect.effect_type == Some(EffectType::Playarounduprank as i32))
            );
        }
        assert_eq!(
            following_steps
                .map(|step| step.act_effect[0].effect_type)
                .collect::<Vec<_>>(),
            [
                Some(EffectType::Deviceskillindex as i32),
                Some(EffectType::Deviceskillindex as i32),
                Some(EffectType::Buffdel as i32),
            ]
        );
        assert!(managers.card.played().iter().all(|card| !card.rewritten));
        assert!(managers.card.resolve_played_ranks().is_empty());
    }
}
