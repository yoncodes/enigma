use super::*;

#[test]
fn parses_retail_episode_pool() {
    assert_eq!(first_number("1102101#1102102#1102103"), Some(1_102_101));
    assert_eq!(first_number(""), None);
}

#[tokio::test]
async fn activity104_markers_round_trip_through_distinct_states() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let episode = config::configs::get()
        .activity104_episode
        .iter()
        .next()
        .unwrap();
    let (activity_id, layer) = (episode.activity_id, episode.layer);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (7, 'act104', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    mark_episode_after_story(&pool, 7, activity_id, layer)
        .await
        .unwrap();
    mark_activity104_story(&pool, 7, activity_id).await.unwrap();
    mark_pop_summary(&pool, 7, activity_id).await.unwrap();
    let info = act104_infos(&pool, 7, Some(activity_id)).await.unwrap();

    assert_eq!(
        info.episodes
            .iter()
            .find(|episode| episode.layer == Some(layer))
            .and_then(|episode| episode.read_after_story),
        Some(true)
    );
    assert_eq!(info.read_activity104_story, Some(true));
    assert_eq!(
        info.pre_summary.and_then(|summary| summary.is_pop_summary),
        Some(true)
    );
}
