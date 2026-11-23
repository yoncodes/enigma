use super::*;
use sqlx::sqlite::SqlitePoolOptions;

async fn test_pool() -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-events', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[test]
fn tower_mop_up_routes_to_current_pass_and_returner_tasks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let targets = event_task_targets(TaskEvent::TowerMopUp { count: 4 }, current_battle_pass_id());

    assert!(
        targets
            .iter()
            .any(|target| target.type_id == TaskType::BpOperAct.id())
    );
    assert!(
        targets
            .iter()
            .any(|target| target.type_id == TaskType::Turnback.id())
    );
}

#[test]
fn production_line_actions_keep_gather_and_create_tasks_distinct() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let gather = event_task_targets(
        TaskEvent::ProductionLine {
            action: ProductionLineAction::Gather,
            count: 3,
        },
        None,
    );
    let create = event_task_targets(
        TaskEvent::ProductionLine {
            action: ProductionLineAction::Create,
            count: 1,
        },
        None,
    );

    assert!(gather.iter().any(|target| target.task_id == 10061));
    assert!(!gather.iter().any(|target| target.task_id == 20101));
    assert!(create.iter().any(|target| target.task_id == 20101));
    assert!(!create.iter().any(|target| target.task_id == 10061));
}

#[test]
fn store_purchase_routes_by_store_id() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let targets = event_task_targets(TaskEvent::StoreGoodsBought { store_id: 111 }, None);
    assert!(targets.iter().any(|target| target.task_id == 180054));
    assert!(
        event_task_targets(TaskEvent::StoreGoodsBought { store_id: 614 }, None)
            .iter()
            .all(|target| target.task_id != 180054)
    );
}

#[test]
fn activity233_routes_only_exact_supported_listeners() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let currency = event_task_targets(
        TaskEvent::CurrencyDec {
            currency_id: 4,
            amount: 1,
        },
        None,
    );
    assert!(currency.iter().any(|target| target.task_id == 790005));
    assert!(!currency.iter().any(|target| target.task_id == 790007));

    let summon = event_task_targets(
        TaskEvent::Summon {
            pool_id: 37111,
            count: 1,
        },
        None,
    );
    assert!(summon.iter().any(|target| target.task_id == 790009));
    assert!(
        !summon
            .iter()
            .any(|target| { target.type_id == TaskType::ActBp.id() && target.task_id >= 790012 })
    );
    assert!(
        !event_task_targets(
            TaskEvent::Summon {
                pool_id: 37112,
                count: 1,
            },
            None,
        )
        .iter()
        .any(|target| target.task_id == 790009)
    );

    let hero_level = event_task_targets(
        TaskEvent::HeroLevelReach {
            hero_id: 3144,
            level: 30,
        },
        None,
    );
    assert!(hero_level.iter().any(|target| target.task_id == 790010));
    assert!(
        event_task_targets(
            TaskEvent::HeroLevelReach {
                hero_id: 3144,
                level: 31,
            },
            None,
        )
        .iter()
        .any(|target| target.task_id == 790010)
    );
    assert!(
        !event_task_targets(
            TaskEvent::HeroLevelReach {
                hero_id: 3144,
                level: 29,
            },
            None,
        )
        .iter()
        .any(|target| target.task_id == 790010)
    );
}

#[test]
fn arcade_win_targets_version_activity_and_captured_pass_tasks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let updated = event_task_targets(
        TaskEvent::Act222ArcadeSettle { activity_id: 13720 },
        Some(26),
    );

    assert!(
        updated
            .iter()
            .any(|task| { task.type_id == TaskType::VersionActivity.id() && task.task_id == 912 })
    );
    assert!(
        updated
            .iter()
            .any(|task| { task.type_id == TaskType::ActBp.id() && task.task_id == 790012 })
    );
    assert!(
        updated
            .iter()
            .any(|task| { task.type_id == TaskType::BattlePass.id() && task.task_id == 103730 })
    );
}

#[test]
fn generic_events_do_not_route_activity_dungeon_tasks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let targets = event_task_targets(
        TaskEvent::CurrencyDec {
            currency_id: 3,
            amount: 1,
        },
        None,
    );
    assert!(
        targets
            .iter()
            .all(|target| target.type_id != TaskType::ActivityDungeon.id())
    );
}

#[tokio::test]
async fn activity233_event_progression_updates_type_79_tasks() {
    let pool = test_pool().await;

    let currency = sync_event_tasks(
        &pool,
        1,
        TaskEvent::CurrencyDec {
            currency_id: 4,
            amount: 500,
        },
    )
    .await
    .unwrap();
    let currency_task = currency.iter().find(|task| task.task_id == 790005).unwrap();
    assert_eq!(currency_task.progress, 500);
    assert!(currency_task.has_finished);

    let summon = sync_event_tasks(
        &pool,
        1,
        TaskEvent::Summon {
            pool_id: 37111,
            count: 1,
        },
    )
    .await
    .unwrap();
    let summon_task = summon.iter().find(|task| task.task_id == 790009).unwrap();
    assert_eq!(summon_task.progress, 1);
    assert_eq!(summon_task.type_id, TaskType::ActBp.id());
}

#[tokio::test]
async fn hero_invitation_claim_sync_updates_only_its_activity_dungeon_tasks() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
             VALUES (1, 1, 0)",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let updated = sync_hero_invitation_claims_in_transaction(&mut tx, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut ids = updated.iter().map(|task| task.task_id).collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(ids, vec![110918, 110919, 110920]);
    assert!(updated.iter().all(|task| {
        task.type_id == TaskType::ActivityDungeon.id() && task.progress == 1 && !task.has_finished
    }));
}
