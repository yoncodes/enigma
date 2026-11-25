use super::{settlement_and_rounds::install_setup_enemy_kill, *};

fn cup_task(task_id: i32, max_progress: i32, progress: i32, status: i32) -> sonettobuf::FightTask {
    sonettobuf::FightTask {
        task_id: Some(task_id),
        status: Some(status),
        values: vec![sonettobuf::FightTaskValue {
            index: Some(1),
            progress: Some(progress),
            max_progress: Some(max_progress),
            finished: Some(progress == max_progress),
        }],
    }
}

#[test]
fn round_deadline_completes_only_for_victory_within_the_limit() {
    crate::test_support::init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    for (round, defeated_uid, completes) in [(8, -1, true), (9, -1, false), (8, 10, false)] {
        let mut runtime = runtime(Fight {
            cur_round: Some(round),
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2)],
                ..Default::default()
            }),
            fight_task_box: Some(sonettobuf::FightTaskBox {
                tasks: vec![
                    cup_task(381_107, 1, 0, 2),
                    cup_task(381_103, 1, 1, 3),
                    cup_task(381_106, 6, 6, 3),
                ],
            }),
            ..Default::default()
        });
        runtime.managers.hp.lose(defeated_uid, 100, 0);

        let reply = runtime
            .build_begin_round_from_schedule(&BeginRoundRequest::default())
            .unwrap();

        assert_eq!(reply.is_finish, Some(true));
        let tasks = runtime.fight.fight_task_box.unwrap().tasks;
        assert_eq!(tasks[0].values[0].finished, Some(completes));
        let notifications = reply
            .fight_step
            .iter()
            .flat_map(|step| &step.act_effect)
            .filter(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Fighttaskupdate as i32)
            })
            .count();
        assert_eq!(notifications, usize::from(completes));
    }
}

#[test]
fn next_round_victory_settles_deadline_after_its_prerequisite() {
    crate::test_support::init_config();
    let entity = |uid, team_type, model_id| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        model_id,
        position: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    fn cup_statuses(steps: &[sonettobuf::FightStep], statuses: &mut Vec<i32>) {
        for effect in steps.iter().flat_map(|step| &step.act_effect) {
            statuses.extend(
                effect
                    .fight_tasks
                    .iter()
                    .filter(|task| task.task_id == Some(381_107))
                    .filter_map(|task| task.status),
            );
            if let Some(step) = &effect.fight_step {
                cup_statuses(std::slice::from_ref(step), statuses);
            }
        }
    }
    for (opcode, timing) in [(105, NoneMode::AfterRoundStart), (106, NoneMode::CardSetup)] {
        let mut runtime = runtime(Fight {
            cur_round: Some(7),
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1, None)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2, Some(120_384_112))],
                ..Default::default()
            }),
            fight_task_box: Some(sonettobuf::FightTaskBox {
                tasks: vec![
                    cup_task(381_107, 1, 0, 1),
                    cup_task(381_103, 1, 1, 3),
                    cup_task(381_106, 6, 5, 2),
                ],
            }),
            ..Default::default()
        });
        install_setup_enemy_kill(&mut runtime, 10, 9_900_070, opcode, timing);
        let reply = runtime
            .build_begin_round_from_schedule(&BeginRoundRequest::default())
            .unwrap();

        assert_eq!(reply.is_finish, Some(true));
        assert_eq!(reply.cur_round, Some(8));
        let mut statuses = Vec::new();
        cup_statuses(&reply.fight_step, &mut statuses);
        assert_eq!(statuses, [2, 3]);
        statuses.clear();
        cup_statuses(&reply.next_round_begin_step, &mut statuses);
        assert!(statuses.is_empty());
    }
}
