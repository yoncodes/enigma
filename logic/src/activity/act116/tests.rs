use super::*;
use database::db::game::currencies;

#[tokio::test]
async fn activity116_progress_persists_and_consumes_configured_costs() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let tables = config::configs::get();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act116-progress', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    currencies::add_currency(&pool, 1, 1202, 1_000)
        .await
        .unwrap();

    let captured_time = 1_790_424_560_190;
    let initial = info(&pool, 1, None, tables, captured_time).await.unwrap();
    assert_eq!(initial.activity_id, Some(11204));
    assert_eq!(
        initial
            .sp_status
            .iter()
            .filter(|status| status.status == Some(1))
            .filter_map(|status| status.episode_id)
            .collect::<Vec<_>>(),
        vec![1270104, 1270105, 1270106]
    );

    let upgrade = upgrade_element(&pool, 1, 11204, 12101021, tables)
        .await
        .unwrap();
    assert_eq!(upgrade.reply.level, Some(1));
    let built = build_trap(&pool, 1, 11204, 10301, tables).await.unwrap();
    assert_eq!(built.reply.trap_id, Some(10301));
    assert!(put_trap(&pool, 1, 11204, 10301, tables).await.is_ok());

    let saved = info(&pool, 1, Some(11204), tables, captured_time)
        .await
        .unwrap();
    assert_eq!(saved.infos[0].level, Some(1));
    assert_eq!(saved.trap_ids, vec![10301]);
    assert_eq!(saved.put_trap, Some(10301));
    assert_eq!(
        currencies::get_currency(&pool, 1, 1202)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        750
    );
    assert!(build_trap(&pool, 1, 11204, 10301, tables).await.is_err());
    assert_eq!(
        currencies::get_currency(&pool, 1, 1202)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        750
    );

    let wrong_activity_id = tables
        .activity
        .iter()
        .find(|activity| activity.type_id != 116)
        .unwrap()
        .id;
    assert!(
        upgrade_element(&pool, 1, wrong_activity_id, 12101021, tables)
            .await
            .is_err()
    );
    assert!(
        build_trap(&pool, 1, wrong_activity_id, 10301, tables)
            .await
            .is_err()
    );
    assert!(
        put_trap(&pool, 1, wrong_activity_id, 10301, tables)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_activity116_state WHERE activity_id = ?"
        )
        .bind(wrong_activity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
