use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn mail_red_dot_only_changes_when_last_unread_mail_is_claimed() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'mail', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_mails (incr_id, user_id, mail_id, create_time, expire_time)
             VALUES (1, 1, 1, 100, 0), (2, 1, 2, 200, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = super::MailManager::new(1);
    let new_mails = manager.created_since(&pool, 100).await.unwrap();
    assert_eq!(new_mails.len(), 1);
    assert_eq!(new_mails[0].incr_id, Some(2));

    let (_, first) = manager.claim_one(&pool, 1).await.unwrap();
    assert_eq!(first.mail_red_dot, None);

    let (_, last) = manager.claim_one(&pool, 2).await.unwrap();
    assert_eq!(last.mail_red_dot.map(|dot| dot.0), Some(0));
}
