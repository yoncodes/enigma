use super::*;

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
            let selection = drain::run_command_group(
                &mut managers,
                &pool,
                &catalog,
                &mut determinism,
                TargetContext::default(),
                [RuleOp::Command(BattleCommand::Conduit(
                    crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                        source_uid: 10,
                        group: initial_group,
                    },
                ))],
            )
            .unwrap();
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
                selection.frames,
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
                    BattleEvent::ActionQueueCommitted { cards, .. } => Some(cards),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(queues.len(), 1);
            assert_eq!(queues[0].len(), 1);
            assert_eq!(
                queues[0][0].card_type,
                Some(sonettobuf::card_info::CardType::Device as i32)
            );
        }
    }
}
