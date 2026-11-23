use super::*;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn bp_info_initializes_the_selected_pass_and_its_tasks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'bp-rollover', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    battle_pass::get_or_create_state(&pool, 1, 28)
        .await
        .unwrap();

    let reply = get_bp_info_for_id(&pool, 1, 26, true).await.unwrap();

    assert_eq!(reply.id, Some(26));
    assert_eq!(reply.score, Some(0));
    assert!(reply.task_info.iter().any(|task| {
        task.r#type == Some(task_db::TaskType::BattlePass.id())
            && config::configs::get()
                .bp_task
                .get(task.id)
                .is_some_and(|config| config.bp_id == 26)
    }));
    assert!(reply.task_info.iter().any(|task| {
        task.r#type == Some(task_db::TaskType::BpOperAct.id())
            && config::configs::get()
                .activity214_task
                .get(task.id)
                .is_some_and(|config| config.bp_id == 26)
    }));
    assert_eq!(reply.start_time, Some(1_786_615_200));
    assert_eq!(reply.end_time, Some(1_790_157_599));
    let ids =
        sqlx::query_scalar::<_, i32>("SELECT bp_id FROM user_battle_pass_state WHERE user_id = 1")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(ids, vec![26]);
}
