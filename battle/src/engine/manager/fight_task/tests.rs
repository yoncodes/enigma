use super::*;

fn task(task_id: i32, max_progress: i32) -> FightTask {
    FightTask {
        task_id: Some(task_id),
        status: Some(RUNNING),
        values: vec![FightTaskValue {
            index: Some(1),
            progress: Some(0),
            max_progress: Some(max_progress),
            finished: Some(false),
        }],
    }
}

fn configured_kill_fight(catalog: crate::catalog::BattleCatalog) -> Fight {
    Fight {
        defender: Some(sonettobuf::FightTeam {
            entitys: [116_370_081, 116_370_082, 116_370_083]
                .into_iter()
                .enumerate()
                .map(|(index, model_id)| sonettobuf::FightEntityInfo {
                    uid: Some(-(index as i64) - 1),
                    model_id: Some(model_id),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        fight_task_box: Some(configured_box(catalog, 493_708).unwrap()),
        ..Default::default()
    }
}

#[test]
fn indicator_updates_emit_changed_tasks_in_box_order() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut manager = FightTaskManager::seed(
        catalog,
        &Fight {
            fight_task_box: Some(FightTaskBox {
                tasks: vec![task(370_701, 5), task(370_703, 3), task(370_702, 1)],
            }),
            ..Default::default()
        },
    );

    manager.record_indicator(3712, 10);
    manager.record_indicator(3711, 1);
    let change = manager.take_pending(1).unwrap();

    assert_eq!(
        change
            .tasks
            .iter()
            .filter_map(|task| task.task_id)
            .collect::<Vec<_>>(),
        vec![370_701, 370_702]
    );
    assert_eq!(change.tasks[0].values[0].progress, Some(1));
    assert_eq!(change.tasks[0].status, Some(RUNNING));
    assert_eq!(change.tasks[1].values[0].progress, Some(1));
    assert_eq!(change.tasks[1].status, Some(FINISHED));
    assert!(manager.take_pending(1).is_none());
}

#[test]
fn teaching_episode_builds_configured_task_states() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());

    let task_box = configured_box(catalog, 493_708).unwrap();

    assert_eq!(
        task_box
            .tasks
            .iter()
            .filter_map(|task| task.task_id)
            .collect::<Vec<_>>(),
        vec![370_801, 370_803, 370_802]
    );
    assert_eq!(task_box.tasks[0].values[0].max_progress, Some(15));
    assert_eq!(task_box.tasks[1].values[0].max_progress, Some(3));
    assert_eq!(task_box.tasks[2].values[0].max_progress, Some(1));
}

#[test]
fn configured_kills_emit_each_matching_death() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut manager = FightTaskManager::seed(catalog, &configured_kill_fight(catalog));

    let first = manager.record_kill(-1, 1).unwrap();
    let second = manager.record_kill(-2, 1).unwrap();
    let third = manager.record_kill(-3, 1).unwrap();

    assert_eq!(first.tasks[0].values[0].progress, Some(1));
    assert_eq!(second.tasks[0].values[0].progress, Some(2));
    assert_eq!(third.tasks[0].values[0].progress, Some(3));
    assert_eq!(third.tasks[0].status, Some(FINISHED));
    assert!(manager.record_kill(-3, 1).is_none());
}

#[test]
fn unsupported_and_malformed_rules_fail_loud() {
    assert!(parse_task_rule(1, "Round#3").is_err());
    assert!(parse_task_rule(1, "Indicator#3713#10#3").is_err());
    assert!(parse_task_rule(1, "KillMonster#bad").is_err());
}

#[test]
fn malformed_snapshot_cannot_mutate_task_progress() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut invalid = Vec::new();

    let mut wrong_max = configured_kill_fight(catalog);
    wrong_max.fight_task_box.as_mut().unwrap().tasks[1].values[0].max_progress = Some(99);
    invalid.push(wrong_max);

    let mut negative_progress = configured_kill_fight(catalog);
    negative_progress.fight_task_box.as_mut().unwrap().tasks[1].values[0].progress = Some(-1);
    invalid.push(negative_progress);

    let mut wrong_finished = configured_kill_fight(catalog);
    wrong_finished.fight_task_box.as_mut().unwrap().tasks[1].values[0].finished = Some(true);
    invalid.push(wrong_finished);

    let mut wrong_status = configured_kill_fight(catalog);
    wrong_status.fight_task_box.as_mut().unwrap().tasks[1].status = Some(FINISHED);
    invalid.push(wrong_status);

    for fight in invalid {
        let mut manager = FightTaskManager::seed(catalog, &fight);
        assert!(manager.record_kill(-1, 1).is_none());
    }
}
