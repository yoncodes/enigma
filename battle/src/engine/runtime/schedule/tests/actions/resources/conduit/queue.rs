use super::*;
use sonettobuf::FightDeviceOper;

#[test]
fn device_only_queue_runs_configured_group_switches() {
    init_config();
    for (initial_group, has_buff, expected_group) in [(2, false, 1), (1, true, 2)] {
        for move_only in [false, true] {
            let fight = Fight {
                attacker: Some(FightTeam {
                    entitys: vec![
                        FightEntityInfo {
                            uid: Some(10),
                            model_id: Some(3145),
                            current_hp: Some(100),
                            passive_skill: vec![31450141],
                            buffs: has_buff
                                .then_some(BuffInfo {
                                    uid: Some(20),
                                    buff_id: Some(31450121),
                                    from_uid: Some(10),
                                    ..Default::default()
                                })
                                .into_iter()
                                .collect(),
                            ..Default::default()
                        },
                        FightEntityInfo {
                            uid: Some(11),
                            model_id: Some(3023),
                            current_hp: Some(100),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                defender: Some(FightTeam {
                    entitys: vec![FightEntityInfo {
                        uid: Some(-1),
                        model_id: Some(1001),
                        current_hp: Some(100),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            };
            let pool = TargetPool::from_fight(&fight);
            let mut managers = BattleManagers::seeded(&fight);
            managers
                .execute_card(CardCommand::Setup(CardSetup {
                    hand: [30230111, 30230121]
                        .into_iter()
                        .map(|skill_id| CardInfo {
                            uid: Some(11),
                            skill_id: Some(skill_id),
                            ..Default::default()
                        })
                        .collect(),
                    draw_pile: Vec::new(),
                    deck_num: 0,
                }))
                .unwrap();
            let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
            let mut determinism = RoundDeterminism::default();
            let result = run_player_phase(
                &fight,
                &mut managers,
                &pool,
                &catalog,
                &mut determinism,
                TargetContext::default(),
                move_only.then_some(RoundCommand::MoveCard {
                    from_index: 0,
                    to_index: 1,
                }),
                &[FightDeviceOper {
                    uid: Some(10),
                    index: Some(initial_group),
                }],
                1,
                crate::engine::manager::emitter::UID,
            )
            .unwrap()
            .drain;

            assert_eq!(managers.conduit.selected_group(10), Some(expected_group));
            assert_eq!(managers.ex_point.get(11), i32::from(move_only));
            let queues = result
                .events
                .iter()
                .filter_map(|event| match event {
                    BattleEvent::ActionQueueCommitted {
                        cards,
                        action_count,
                        ..
                    } => Some((cards, action_count)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(queues.len(), 1);
            assert!(queues[0].0.is_empty());
            assert_eq!(*queues[0].1, 1);

            let owner_index = |owner: FrameOwner| {
                result
                    .frames
                    .iter()
                    .position(|frame| frame.owner == owner)
                    .unwrap()
            };
            assert!(
                owner_index(FrameOwner::PlayerActionCommit)
                    < owner_index(FrameOwner::ConduitSelectionPublication)
            );
            assert!(
                owner_index(FrameOwner::ConduitSelectionPublication)
                    < owner_index(FrameOwner::ConduitSelectionConfirmation)
            );

            let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
            let effects = steps
                .iter()
                .flat_map(|step| &step.act_effect)
                .collect::<Vec<_>>();
            let use_cards = effects
                .iter()
                .find(|effect| {
                    effect.effect_type
                        == Some(sonettobuf::effect_type_enum::EffectType::Usecards as i32)
                })
                .unwrap();
            assert_eq!(use_cards.card_info_list.len(), 1);
            assert_eq!(
                use_cards.card_info_list[0].card_type,
                Some(sonettobuf::card_info::CardType::Device as i32)
            );
            let selection_steps = steps
                .iter()
                .filter(|step| {
                    step.act_effect.first().is_some_and(|effect| {
                        effect.effect_type
                            == Some(
                                sonettobuf::effect_type_enum::EffectType::Deviceskillindex as i32,
                            )
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(selection_steps.len(), 2);
            for step in selection_steps {
                assert_eq!(step.act_effect.len(), 1);
                assert_eq!(step.act_effect[0].target_id, Some(10));
                assert_eq!(step.act_effect[0].effect_num, Some(expected_group));
                assert_eq!(step.act_effect[0].config_effect, Some(0));
            }
        }
    }
}
