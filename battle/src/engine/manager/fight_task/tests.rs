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
        episode_id: Some(493_708),
        fight_task_box: Some(configured_box(catalog, 493_708).unwrap()),
        ..Default::default()
    }
}

fn configured_task_mut(fight: &mut Fight, task_id: i32) -> &mut FightTask {
    fight
        .fight_task_box
        .as_mut()
        .unwrap()
        .tasks
        .iter_mut()
        .find(|task| task.task_id == Some(task_id))
        .unwrap()
}

#[test]
fn indicator_updates_emit_changed_tasks_in_box_order() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut manager = FightTaskManager::seed(
        catalog,
        &Fight {
            episode_id: Some(493_707),
            fight_task_box: Some(FightTaskBox {
                tasks: vec![task(370_701, 5), task(370_703, 3), task(370_702, 1)],
            }),
            ..Default::default()
        },
    )
    .unwrap();

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
fn teaching_episode_builds_capture_ordered_task_states() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());

    for (episode_id, expected) in [
        (493_701, vec![370_101, 370_103, 370_102]),
        (493_702, vec![370_201, 370_203, 370_202]),
        (493_707, vec![370_701, 370_703, 370_702]),
        (493_708, vec![370_801, 370_803, 370_802]),
    ] {
        assert_eq!(
            configured_box(catalog, episode_id)
                .unwrap()
                .tasks
                .iter()
                .filter_map(|task| task.task_id)
                .collect::<Vec<_>>(),
            expected
        );
    }
    let task_box = configured_box(catalog, 493_708).unwrap();
    assert_eq!(task_box.tasks[0].values[0].max_progress, Some(15));
    assert_eq!(task_box.tasks[1].values[0].max_progress, Some(3));
    assert_eq!(task_box.tasks[2].values[0].max_progress, Some(1));
}

#[test]
fn configured_kills_emit_each_matching_death() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut manager = FightTaskManager::seed(catalog, &configured_kill_fight(catalog)).unwrap();

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
fn invalid_task_snapshots_are_rejected() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    for invalid_task in [
        FightTask {
            task_id: None,
            ..Default::default()
        },
        task(i32::MAX, 1),
        task(26_004, 1),
    ] {
        let fight = Fight {
            episode_id: Some(493_708),
            fight_task_box: Some(FightTaskBox {
                tasks: vec![invalid_task],
            }),
            ..Default::default()
        };
        assert!(FightTaskManager::seed(catalog, &fight).is_err());
    }
}

#[test]
fn invalid_task_sequences_are_rejected() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut invalid = Vec::new();

    let mut missing_box = configured_kill_fight(catalog);
    missing_box.fight_task_box = None;
    invalid.push(missing_box);

    let mut omitted = configured_kill_fight(catalog);
    omitted.fight_task_box.as_mut().unwrap().tasks.pop();
    invalid.push(omitted);

    let mut reordered = configured_kill_fight(catalog);
    reordered.fight_task_box.as_mut().unwrap().tasks.swap(1, 2);
    invalid.push(reordered);

    let mut duplicate = configured_kill_fight(catalog);
    let tasks = &mut duplicate.fight_task_box.as_mut().unwrap().tasks;
    tasks[2] = tasks[1].clone();
    invalid.push(duplicate);

    let mut foreign = configured_kill_fight(catalog);
    foreign.fight_task_box = Some(configured_box(catalog, 493_707).unwrap());
    invalid.push(foreign);

    for fight in invalid {
        assert!(FightTaskManager::seed(catalog, &fight).is_err());
    }
}

#[test]
fn malformed_snapshot_is_rejected() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut invalid = Vec::new();

    let mut wrong_max = configured_kill_fight(catalog);
    configured_task_mut(&mut wrong_max, 370_803).values[0].max_progress = Some(99);
    invalid.push(wrong_max);

    let mut negative_progress = configured_kill_fight(catalog);
    configured_task_mut(&mut negative_progress, 370_803).values[0].progress = Some(-1);
    invalid.push(negative_progress);

    let mut wrong_finished = configured_kill_fight(catalog);
    configured_task_mut(&mut wrong_finished, 370_803).values[0].finished = Some(true);
    invalid.push(wrong_finished);

    let mut missing_progress = configured_kill_fight(catalog);
    configured_task_mut(&mut missing_progress, 370_803).values[0].progress = None;
    invalid.push(missing_progress);

    let mut missing_finished = configured_kill_fight(catalog);
    configured_task_mut(&mut missing_finished, 370_803).values[0].finished = None;
    invalid.push(missing_finished);

    let mut wrong_status = configured_kill_fight(catalog);
    configured_task_mut(&mut wrong_status, 370_803).status = Some(FINISHED);
    invalid.push(wrong_status);

    for fight in invalid {
        assert!(FightTaskManager::seed(catalog, &fight).is_err());
    }
}
