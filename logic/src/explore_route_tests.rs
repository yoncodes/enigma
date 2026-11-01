//! End-to-end route using the uploaded map, with battles settled in memory.

use database::db::game::{battle as battle_db, explore_runtime};

#[tokio::test]
async fn first_map_route_preserves_progress_and_unlocks_its_successors() {
    crate::init_test_server_config();
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 991001_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-first-map-route").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 47, 1)")
        .bind(user_id).execute(&pool).await.unwrap();
    crate::explore_entry::change_map(&pool, user_id, 101).await.unwrap();
    let route: Vec<serde_json::Value> = serde_json::from_str(include_str!("../tests/fixtures/explore101-route.json")).unwrap();
    for (index, action) in route.iter().enumerate() {
        let kind = action[0].as_str().unwrap();
        let id = action[1].as_i64().unwrap() as i32;
        let result: anyhow::Result<()> = match kind {
            "move" => crate::explore_move::move_player(&pool, user_id, id, action[2].as_i64().unwrap() as i32).await.map(|_| ()),
            "battle" => async {
                let crate::explore_battle::BattleInteraction::Begin { battle_id, .. } =
                    crate::explore_battle::interact(&pool, user_id, id).await? else {
                        anyhow::bail!("expected a new encounter");
                    };
                let pending = crate::explore_battle::pending(&pool, user_id).await?.unwrap();
                let mut tx = pool.begin().await?;
                let fight_id = battle_db::create_fight_instance_in_transaction(&mut tx, battle_db::NewFightInstance {
                    user_id, episode_id: 140101, battle_id, multiplication: 1,
                    entry_cost: "{}", checkpoint: "{}", created_at: 0,
                }).await?;
                crate::explore_battle::bind_fight(&mut tx, user_id, &pending, fight_id).await?;
                tx.commit().await?;
                // The battle engine itself is tested separately; this marks a synthetic victory.
                crate::explore_battle::settle(&pool, user_id, fight_id, battle_id, true).await?;
                match crate::explore_battle::interact(&pool, user_id, id).await? {
                    crate::explore_battle::BattleInteraction::Finish { .. } => Ok(()),
                    _ => anyhow::bail!("encounter did not finish"),
                }
            }.await,
            "chest" => async {
                let claim = crate::explore_chest::claim(&pool, user_id, id).await?;
                let uid = claim.items.first().and_then(|item| item.uid).ok_or_else(|| anyhow::anyhow!("rune reward missing"))?;
                crate::explore_rune::select(&pool, user_id, uid).await?;
                Ok(())
            }.await,
            "rune" => crate::explore_rune::swap(&pool, user_id, id).await.map(|_| ()),
            "teleport" => crate::explore_teleport::transfer(&pool, user_id, id).await.map(|_| ()),
            "archive" => crate::explore_engine::collect_basic_archive(&pool, user_id, id).await.map(|_| ()),
            "exit" => crate::explore_exit::complete(&pool, user_id, id).await.map(|_| ()),
            _ => Err(anyhow::anyhow!("unknown test route action")),
        };
        result.unwrap_or_else(|error| panic!("route action {index} {action}: {error:#}"));
        if index == 75 || index == 120 {
            let info = crate::explore_entry::current_info(&pool, user_id).await.unwrap();
            let map = info.explore_info.unwrap().explore_map.unwrap();
            assert_eq!(map.map_id, Some(101));
        }
    }
    let unlocked: Vec<i32> = sqlx::query_scalar("SELECT map_id FROM user_explore_unlocked_maps WHERE user_id = ? ORDER BY map_id")
        .bind(user_id).fetch_all(&pool).await.unwrap();
    assert_eq!(unlocked, vec![101, 102, 201, 301]);
    let star: i32 = sqlx::query_scalar("SELECT star FROM user_dungeons WHERE user_id = ? AND episode_id = 140101")
        .bind(user_id).fetch_one(&pool).await.unwrap();
    assert_eq!(star, 1);
    let mut tx = pool.begin().await.unwrap();
    let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
    assert_eq!(saved.objects[&431].status & 1, 0);
    assert_eq!(saved.objects[&428].status & 1, 0);
    assert_eq!(saved.objects[&394].status & 4, 4);
    assert_eq!(saved.challenge_count, 1);
    let items = explore_runtime::list_items(&mut tx, user_id).await.unwrap();
    assert_eq!(items[0].status, Some(1));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn second_map_route_fetches_a_rock_and_holds_open_the_first_door() {
    crate::init_test_server_config();
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 991002_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-second-map-route").execute(&pool).await.unwrap();
    let layout = config::explore_layout::layout(102).unwrap().unwrap();
    let map = explore_runtime::MapState::new(layout);
    let mut tx = pool.begin().await.unwrap();
    explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
    tx.commit().await.unwrap();
    // The route crosses timed traps while they are inactive; trap hits have a
    // separate isolated test and would return the player to their checkpoint.
    for x in 2..=20 {
        crate::explore_move::move_player(&pool, user_id, x, 2).await
            .unwrap_or_else(|e| panic!("second map approach ({x},2): {e:#}"));
    }
    crate::explore_move::move_player(&pool, user_id, 20, 3).await.unwrap();
    let rock = crate::explore_rock::take(&pool, user_id, 26).await.unwrap();
    let uid = rock.item.uid.unwrap();
    for (x,y) in [(19,3),(18,3),(18,2),(17,2),(16,2),(15,2),(14,2),(13,2),(12,2),
        (12,3),(11,3),(10,3),(9,3),(8,3),(8,4),(7,4),(7,5)] {
        crate::explore_move::move_player(&pool, user_id, x, y).await
            .unwrap_or_else(|e| panic!("second map return ({x},{y}): {e:#}"));
    }
    crate::explore_rock::place(&pool, user_id, uid, 6, 5).await.unwrap();
    crate::explore_move::move_player(&pool, user_id, 7, 6).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let saved = explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap().state;
    assert_eq!((saved.x, saved.y), (7, 6));
    assert_eq!((saved.objects[&14].status & 4, saved.objects[&2].status & 4), (4,4));
    assert_eq!((saved.objects[&(uid as i32)].x, saved.objects[&(uid as i32)].y), (6,5));
    assert_eq!(saved.challenge_count, 0);
    assert!(explore_runtime::list_items(&mut tx, user_id).await.unwrap().is_empty());
    tx.commit().await.unwrap();
}
