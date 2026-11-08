use super::*;
use database::db::game::currencies;

async fn database(player_id: i64, username: &str) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, ?, 0, 0)",
    )
    .bind(player_id)
    .bind(username)
    .execute(&db)
    .await
    .unwrap();
    db
}

fn racing_activity_id() -> i32 {
    config::configs::get()
        .racing_gift
        .iter()
        .next()
        .unwrap()
        .activity_id
}

#[tokio::test]
async fn info_projects_configured_episodes_and_free_racers() {
    let db = database(70, "act243-info").await;
    let activity_id = racing_activity_id();
    let reply = info(&db, 70, activity_id).await.unwrap();

    assert_eq!(reply.activity_id, Some(activity_id));
    assert_eq!(
        reply.episodes.len(),
        config::configs::get()
            .activity243_episode
            .iter()
            .filter(|row| row.activity_id == activity_id)
            .count()
    );
    assert!(reply.episodes.iter().all(|episode| {
        episode.is_finished == Some(false)
            && episode.best_score == Some(0)
            && episode.best_star == Some(0)
    }));
    let free_racers = config::configs::get()
        .racing_racer
        .iter()
        .filter(|racer| racer.display == 1 && racer.cost == 0)
        .map(|racer| racer.id)
        .collect::<Vec<_>>();
    assert_eq!(reply.growth.unwrap().unlocked_racers, free_racers);

    let push = info_push(&db, 70, activity_id).await.unwrap();
    assert_eq!(push.activity_id, reply.activity_id);
    assert_eq!(push.episodes, reply.episodes);
}

#[tokio::test]
async fn report_preserves_bests_and_claims_only_new_star_rewards() {
    let db = database(71, "act243-report").await;
    let activity_id = racing_activity_id();
    let episodes = config::configs::get()
        .activity243_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .collect::<Vec<_>>();
    let first = episodes[0];
    let second = episodes[1];

    assert!(
        report_episode(
            &db,
            71,
            activity_id,
            second.episode_id,
            0,
            1,
            900,
            "100".into(),
        )
        .await
        .is_err()
    );

    let mut expected = reward::parse_reward_id(first.clear_bonus);
    for row in config::configs::get()
        .racing_reward
        .iter()
        .filter(|row| row.episode_id == first.episode_id && row.star <= 2)
    {
        expected.extend(reward::parse_reward_id(row.bonus));
    }
    let report = report_episode(
        &db,
        71,
        activity_id,
        first.episode_id,
        100,
        2,
        900,
        "110".into(),
    )
    .await
    .unwrap();
    let episode = report.reply.episode.unwrap();
    assert_eq!(episode.claimed_stars, [1, 2]);
    assert_eq!(episode.best_score, Some(100));
    assert_eq!(episode.best_star, Some(2));
    assert_eq!(episode.best_time_ms, Some(900));
    assert_eq!(episode.record.as_deref(), Some("110"));
    assert_eq!(report.material_changes, expected.material_changes());

    let report = report_episode(
        &db,
        71,
        activity_id,
        first.episode_id,
        50,
        1,
        1200,
        "100".into(),
    )
    .await
    .unwrap();
    let episode = report.reply.episode.unwrap();
    assert_eq!(report.reply.is_new_best, Some(false));
    assert_eq!(episode.claimed_stars, [1, 2]);
    assert_eq!(episode.best_score, Some(100));
    assert_eq!(episode.best_star, Some(2));
    assert_eq!(episode.best_time_ms, Some(900));
    assert_eq!(episode.record.as_deref(), Some("110"));
    assert_eq!(
        report.material_changes,
        reward::parse_reward_id(first.clear_bonus).material_changes()
    );

    let report = report_episode(
        &db,
        71,
        activity_id,
        first.episode_id,
        0,
        1,
        800,
        "100".into(),
    )
    .await
    .unwrap();
    let episode = report.reply.episode.unwrap();
    assert_eq!(episode.best_star, Some(2));
    assert_eq!(episode.best_time_ms, Some(800));
    assert_eq!(episode.record.as_deref(), Some("110"));

    let report = report_episode(
        &db,
        71,
        activity_id,
        first.episode_id,
        0,
        2,
        700,
        "101".into(),
    )
    .await
    .unwrap();
    let episode = report.reply.episode.unwrap();
    assert_eq!(episode.best_star, Some(3));
    assert_eq!(episode.best_time_ms, Some(700));
    assert_eq!(episode.record.as_deref(), Some("111"));
}

#[tokio::test]
async fn growth_costs_and_unlock_order_are_config_owned() {
    let db = database(72, "act243-growth").await;
    let activity_id = racing_activity_id();
    let currency_id = currency_id(activity_id).unwrap();
    currencies::add_currency(&db, 72, currency_id, 10_000)
        .await
        .unwrap();
    let mut gifts = config::configs::get()
        .racing_gift
        .iter()
        .filter(|row| row.activity_id == activity_id && row.level == 1)
        .collect::<Vec<_>>();
    gifts.sort_by_key(|row| (row.group, row.order));
    let first = gifts[0];
    let second = gifts
        .iter()
        .find(|row| row.group == first.group && row.order == first.order + 1)
        .unwrap();
    assert!(
        upgrade_gift(&db, 72, activity_id, second.gift_point)
            .await
            .is_err()
    );

    let upgraded = upgrade_gift(&db, 72, activity_id, first.gift_point)
        .await
        .unwrap();
    assert_eq!(upgraded.reply.gift.unwrap().level, Some(1));
    assert_eq!(upgraded.consumed.currency_ids, [(currency_id, -first.cost)]);
    assert!(
        upgrade_gift(&db, 72, activity_id, first.gift_point)
            .await
            .is_err()
    );

    let racer = config::configs::get()
        .racing_racer
        .iter()
        .find(|racer| racer.display == 1 && racer.cost > 0)
        .unwrap();
    let unlocked = unlock_racer(&db, 72, activity_id, racer.id).await.unwrap();
    assert_eq!(unlocked.reply.racer_id, Some(racer.id));
    assert_eq!(unlocked.consumed.currency_ids, [(currency_id, -racer.cost)]);
    assert!(unlock_racer(&db, 72, activity_id, racer.id).await.is_err());

    let growth = info(&db, 72, activity_id).await.unwrap().growth.unwrap();
    assert!(growth.unlocked_racers.contains(&racer.id));
    assert!(
        growth
            .gifts
            .iter()
            .any(|gift| gift.gift_point == Some(first.gift_point) && gift.level == Some(1))
    );
}
