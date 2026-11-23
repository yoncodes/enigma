use super::*;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

async fn test_pool(player_id: i64) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
    )
    .bind(player_id)
    .bind(format!("hero-invitation-{player_id}"))
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn finish_element(pool: &SqlitePool, player_id: i64, element_id: i32) {
    sqlx::query(
        "INSERT INTO user_dungeon_elements
             (user_id, element_id, is_finished, puzzle_progress, puzzle_updated_at)
             VALUES (?, ?, 1, '', 0)",
    )
    .bind(player_id)
    .bind(element_id)
    .execute(pool)
    .await
    .unwrap();
}

#[test]
fn slash_config_times_are_supported_for_invitation_rows() {
    assert!(common::time::ServerTime::config_datetime_sec("2024/05/19 05:00:00").is_some());
    assert!(!map_element_condition_met(
        "ChapterMapElement=311101",
        &HashSet::new()
    ));
}

#[tokio::test]
async fn projection_follows_finished_element_chain_and_claim_markers() {
    let player_id = 5631;
    let pool = test_pool(player_id).await;
    for invitation in config::configs::get().hero_invitation.iter() {
        finish_element(&pool, player_id, invitation.element_id).await;
        sqlx::query(
            "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
                 VALUES (?, ?, 0)",
        )
        .bind(player_id)
        .bind(invitation.id)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
             VALUES (?, 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let info = HeroInvitationManager::new(player_id)
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(info.opened_invite, (1..=14).collect::<Vec<_>>());
    assert_eq!(info.gain_reward, (1..=14).collect::<Vec<_>>());
    assert_eq!(info.final_reward, Some(true));
}

#[tokio::test]
async fn individual_claim_is_idempotent_and_final_claim_is_gated() {
    let player_id = 5632;
    let pool = test_pool(player_id).await;
    finish_element(&pool, player_id, 311101).await;
    let manager = HeroInvitationManager::new(player_id);

    assert!(matches!(
        manager
            .gain_final_reward(&pool, config::configs::get())
            .await,
        Err(AppError::InvalidRequest)
    ));
    let first = manager
        .gain_reward(&pool, config::configs::get(), 1)
        .await
        .unwrap();
    assert!(first.newly_claimed);
    assert_eq!(first.rewards.item_ids, vec![120012]);
    let second = manager
        .gain_reward(&pool, config::configs::get(), 1)
        .await
        .unwrap();
    assert!(!second.newly_claimed);
    assert!(second.rewards.item_ids.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn concurrent_individual_claims_grant_the_reward_once() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let url = format!(
        "sqlite:file:hero-invitation-claim-{}?mode=memory&cache=shared",
        common::time::ServerTime::now_ms()
    );
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5636;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'hero-invitation-concurrent', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    finish_element(&pool, player_id, 311101).await;

    let manager = HeroInvitationManager::new(player_id);
    let (left, right) = tokio::join!(
        manager.gain_reward(&pool, config::configs::get(), 1),
        manager.gain_reward(&pool, config::configs::get(), 1)
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(
        usize::from(left.newly_claimed) + usize::from(right.newly_claimed),
        1
    );
    assert_eq!(left.updated_tasks.len() + right.updated_tasks.len(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM items WHERE user_id = ? AND item_id = 120012",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_tasks
                 WHERE user_id = ? AND type_id = 11 AND task_id = 110918",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_hero_invitation_claims
                 WHERE user_id = ? AND invite_id = 1",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn invalid_reward_rolls_back_the_claim_marker() {
    let player_id = 5633;
    let pool = test_pool(player_id).await;
    finish_element(&pool, player_id, 311101).await;
    sqlx::query(
        "CREATE TRIGGER fail_hero_invitation_reward
             BEFORE INSERT ON items
             WHEN NEW.user_id = 5633 AND NEW.item_id = 120012
             BEGIN SELECT RAISE(ABORT, 'test reward failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        HeroInvitationManager::new(player_id)
            .gain_reward(&pool, config::configs::get(), 1)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn task_sync_failure_rolls_back_the_claim_and_reward() {
    let player_id = 5637;
    let pool = test_pool(player_id).await;
    finish_element(&pool, player_id, 311101).await;
    sqlx::query(
        "CREATE TRIGGER fail_hero_invitation_task
             BEFORE INSERT ON user_tasks
             WHEN NEW.user_id = 5637 AND NEW.type_id = 11
             BEGIN SELECT RAISE(ABORT, 'test task failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        HeroInvitationManager::new(player_id)
            .gain_reward(&pool, config::configs::get(), 1)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM items WHERE user_id = ? AND item_id = 120012",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn final_claim_uses_the_configured_const_reward_after_all_invites() {
    let player_id = 5634;
    let pool = test_pool(player_id).await;
    for invitation in config::configs::get().hero_invitation.iter() {
        finish_element(&pool, player_id, invitation.element_id).await;
        sqlx::query(
            "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
                 VALUES (?, ?, 0)",
        )
        .bind(player_id)
        .bind(invitation.id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let claim = HeroInvitationManager::new(player_id)
        .gain_final_reward(&pool, config::configs::get())
        .await
        .unwrap();
    assert!(claim.newly_claimed);
    assert_eq!(claim.rewards.skin_gains[0].skin_id, 302304);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_hero_invitation_claims
                 WHERE user_id = ? AND invite_id = 0",
        )
        .bind(player_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
