use database::db::game::open_infos;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn open_47(pool: &SqlitePool, user_id: i64) -> bool {
    open_infos::get_open_info(pool, user_id, 47)
        .await
        .expect("read open state")
        .is_open
}

#[tokio::test]
async fn third_door_requires_the_cleared_episode_and_finished_element() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("data/excel2json");
    config::init(data.to_str().expect("valid data path")).expect("load game configuration");

    let tables = config::configs::get();
    let open = tables.open.get(47).expect("feature 47 exists");
    let episode = tables.episode.get(10503).expect("episode 10503 exists");
    assert_eq!(open.episode_id, 503);
    assert_eq!(episode.chain_episode, 503);
    assert_eq!(open.element_id, 1050302);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create isolated database");
    database::run_migrations(&pool).await.expect("migrate isolated database");
    let user_id = 900001_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id)
        .bind("isolated-feature-test")
        .execute(&pool)
        .await
        .expect("create isolated user");
    sqlx::query(
        "INSERT INTO user_dungeons (user_id, chapter_id, episode_id, star, created_at, updated_at) \
         VALUES (?, 105, 10503, 0, 0, 0)",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("create unfinished episode");
    sqlx::query(
        "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished) VALUES (?, 1050302, 1)",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("create finished element");

    open_infos::reconcile_progression(&pool, user_id).await.expect("reconcile unfinished episode");
    assert!(!open_47(&pool, user_id).await, "an unfinished episode must not open feature 47");

    sqlx::query("UPDATE user_dungeons SET star = 1 WHERE user_id = ? AND episode_id = 10503")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("complete episode");
    sqlx::query("UPDATE user_dungeon_elements SET is_finished = 0 WHERE user_id = ? AND element_id = 1050302")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("mark element unfinished");
    open_infos::reconcile_progression(&pool, user_id).await.expect("reconcile unfinished element");
    assert!(!open_47(&pool, user_id).await, "an unfinished element must not open feature 47");

    sqlx::query("UPDATE user_dungeon_elements SET is_finished = 1 WHERE user_id = ? AND element_id = 1050302")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("complete element");
    open_infos::reconcile_progression(&pool, user_id).await.expect("reconcile finished progression");
    assert!(open_47(&pool, user_id).await, "the cleared episode and finished element must open feature 47");
}
