use super::*;

#[tokio::test]
async fn malformed_chapter_state_is_not_replaced() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let chapter = chapter_ids(tables)[0];
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (43, 'dice-hero-corrupt', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    dice_hero::sync_chapters(&db, 43, vec![chapter])
        .await
        .unwrap();

    sqlx::query(
        "UPDATE user_dice_hero_chapters SET pass_level_ids = '{'
             WHERE user_id = 43 AND chapter = ?",
    )
    .bind(chapter)
    .execute(&db)
    .await
    .unwrap();
    assert!(
        dice_hero::complete_level(&db, 43, chapter, 1)
            .await
            .is_err()
    );

    for field in [
        "pass_level_ids",
        "reward_items_json",
        "relic_ids",
        "skill_card_ids",
    ] {
        sqlx::query(
            "UPDATE user_dice_hero_chapters
                 SET pass_level_ids = '[]', reward_items_json = '[]',
                     relic_ids = '[]', skill_card_ids = '[]'
                 WHERE user_id = 43 AND chapter = ?",
        )
        .bind(chapter)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(&format!(
            "UPDATE user_dice_hero_chapters SET {field} = '{{'
                 WHERE user_id = 43 AND chapter = ?"
        ))
        .bind(chapter)
        .execute(&db)
        .await
        .unwrap();

        assert!(matches!(
            dice_hero_get_reward(&db, 43, chapter, Vec::new(), tables).await,
            Err(AppError::Serde(_))
        ));
        let stored: String = sqlx::query_scalar(&format!(
            "SELECT {field} FROM user_dice_hero_chapters
                 WHERE user_id = 43 AND chapter = ?"
        ))
        .bind(chapter)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(stored, "{", "{field}");
    }
}
