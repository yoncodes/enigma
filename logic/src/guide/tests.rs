use super::*;
use database::db::game::dungeons;

#[tokio::test]
async fn finish_guide_persists_step() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE guide_progress (
                user_id INTEGER NOT NULL,
                guide_id INTEGER NOT NULL,
                step_id INTEGER NOT NULL,
                PRIMARY KEY(user_id, guide_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let configured_step = config::configs::get()
        .guide_step
        .iter()
        .find(|step| {
            story_requirement(&step.action).is_none()
                && config::configs::get().guide_step.iter().any(|later| {
                    later.id == step.id && later.step_id > step.step_id && later.key_step != 0
                })
        })
        .unwrap();
    let guide_id = configured_step.id;
    let expected_step_id = configured_step.step_id;
    GuideManager::new(7)
        .finish(&pool, guide_id, expected_step_id)
        .await
        .unwrap();
    let stored_step_id: i32 =
        sqlx::query_scalar("SELECT step_id FROM guide_progress WHERE user_id = 7 AND guide_id = ?")
            .bind(guide_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(stored_step_id, expected_step_id);

    assert!(
        config::configs::get()
            .guide_step
            .iter()
            .all(|step| step.id != 102 || step.step_id != 0)
    );
    GuideManager::new(8).finish(&pool, 102, 0).await.unwrap();
    let start_marker: i32 = sqlx::query_scalar(
        "SELECT step_id FROM guide_progress WHERE user_id = 8 AND guide_id = 102",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(start_marker, 0);
}

#[tokio::test]
async fn complete_all_marks_every_online_guide_finished() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE guide_progress (
            user_id INTEGER NOT NULL,
            guide_id INTEGER NOT NULL,
            step_id INTEGER NOT NULL,
            PRIMARY KEY(user_id, guide_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let completed = GuideManager::new(77).complete_all(&pool).await.unwrap();
    assert!(!completed.is_empty());
    assert!(completed.iter().all(|guide| guide.step_id == -1));
    assert_eq!(
        guides::get_all_guide_progress(&pool, 77)
            .await
            .unwrap()
            .len(),
        completed.len()
    );
}

#[test]
fn hero_reward_is_derived_from_the_following_guide_step() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert_eq!(hero_reward_after_step(101, 84), Some(3028));
    assert_eq!(hero_reward_after_step(i32::MAX, i32::MAX), None);
}

#[tokio::test]
async fn prologue_guide_grants_apple_after_story_once() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (8, 'prologue', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 8)
        .await
        .unwrap();

    let heroes = UserHeroModel::new(8, pool.clone());
    assert_eq!(
        GuideManager::new(8)
            .get_info(&pool)
            .await
            .unwrap()
            .guide_infos,
        [GuideInfo {
            guide_id: 101,
            step_id: 0,
        }]
    );
    assert!(heroes.get_all_heroes().await.unwrap().is_empty());
    assert!(matches!(
        GuideManager::new(8).finish(&pool, 101, 84).await,
        Err(AppError::InvalidRequest)
    ));

    crate::story::StoryManager::new(8)
        .update(&pool, 100017, -1, 0)
        .await
        .unwrap();
    assert!(heroes.get_all_heroes().await.unwrap().is_empty());

    let completion = GuideManager::new(8).finish(&pool, 101, 84).await.unwrap();
    let apple = heroes.get_hero(3028).await.unwrap();
    assert_eq!(completion.rewards.hero_ids, [3028]);
    assert_eq!(completion.rewards.item_ids, [140001]);
    assert_eq!(completion.guide_info.step_id, -1);
    assert_eq!(
        completion
            .group_snapshot
            .unwrap()
            .group_info
            .unwrap()
            .hero_list,
        [apple.record.uid]
    );

    let repeated = GuideManager::new(8).finish(&pool, 101, 84).await.unwrap();
    assert!(repeated.rewards.hero_ids.is_empty());
    assert_eq!(heroes.get_all_heroes().await.unwrap().len(), 1);
}

#[tokio::test]
async fn skip_initial_tutorial_applies_its_configured_progress_and_rewards_once() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (9, 'skip-prologue', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 9)
        .await
        .unwrap();

    let manager = GuideManager::new(9);
    manager.skip_initial_tutorial(&pool).await.unwrap();
    manager.skip_initial_tutorial(&pool).await.unwrap();

    assert_eq!(
        guides::get_guide_progress(&pool, 9, 101)
            .await
            .unwrap()
            .unwrap()
            .step_id,
        -1
    );
    assert_eq!(
        guides::get_guide_progress(&pool, 9, 103)
            .await
            .unwrap()
            .unwrap()
            .step_id,
        -1
    );
    assert_eq!(
        guides::get_guide_progress(&pool, 9, 102)
            .await
            .unwrap()
            .unwrap()
            .step_id,
        -1
    );
    for episode_id in [10001, 10002, 10003] {
        assert_eq!(
            dungeons::episode_star(&pool, 9, episode_id).await.unwrap(),
            1
        );
    }
    assert_eq!(dungeons::episode_star(&pool, 9, 10101).await.unwrap(), 2);
    assert!(stories::is_story_finished(&pool, 9, 100017).await.unwrap());
    assert!(stories::is_story_finished(&pool, 9, 100009).await.unwrap());
    assert!(stories::is_story_finished(&pool, 9, 100010).await.unwrap());
    let hero_ids = UserHeroModel::new(9, pool.clone())
        .get_all_heroes()
        .await
        .unwrap()
        .into_iter()
        .map(|hero| hero.record.hero_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(hero_ids, [3023, 3028].into_iter().collect());
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM items WHERE user_id = 9 AND item_id = 140001",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT exp FROM users WHERE id = 9")
            .fetch_one(&pool)
            .await
            .unwrap(),
        80
    );
}

#[tokio::test]
async fn skip_initial_tutorial_finishes_a_partially_completed_stage() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (10, 'partial-prologue', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 10)
        .await
        .unwrap();
    let episode = config::configs::get().episode.get(10101).unwrap();
    dungeons::update_dungeon_progress(&pool, 10, episode.chapter_id, episode.id, 1)
        .await
        .unwrap();

    GuideManager::new(10)
        .skip_initial_tutorial(&pool)
        .await
        .unwrap();

    assert_eq!(dungeons::episode_star(&pool, 10, 10101).await.unwrap(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT exp FROM users WHERE id = 10")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
