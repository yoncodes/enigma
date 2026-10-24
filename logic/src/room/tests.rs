use super::*;
use database::models::game::block_packages::BlockInfo as StoredBlockInfo;
use sqlx::sqlite::SqlitePoolOptions;

async fn room_test_pool(user_id: i64, username: &str) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id)
        .bind(username)
        .execute(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn room_hero_update_replaces_owned_placement() {
    let pool = room_test_pool(13, "room-heroes").await;
    database::db::starter_data::load_all_starter_data(&pool, 13)
        .await
        .unwrap();
    UserHeroModel::new(13, pool.clone())
        .create_hero(3023)
        .await
        .unwrap();

    let initial_blocks = block_packages::get_blocks(&pool, 13).await.unwrap();
    assert_eq!(
        initial_blocks
            .iter()
            .map(|block| block.block_id)
            .collect::<Vec<_>>(),
        vec![-6, -5, -4, -3, -2, -1]
    );
    assert_eq!(
        block_packages::get_block_packages(&pool, 13)
            .await
            .unwrap()
            .into_iter()
            .map(|package| package.block_package_id)
            .collect::<Vec<_>>(),
        vec![6]
    );
    assert_eq!(
        room_plan_building_degree(
            &pool,
            config::configs::get(),
            13,
            &initial_blocks
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>(),
            &[],
        )
        .await
        .unwrap(),
        48
    );
    assert_eq!(
        RoomManager::new(13)
            .reset_room(&pool)
            .await
            .unwrap()
            .infos
            .len(),
        6
    );

    let rooms = RoomManager::new(13);
    let placed = rooms
        .update_room_hero_data(&pool, config::configs::get(), &[3023])
        .await
        .unwrap();
    assert_eq!(placed.room_hero_datas.len(), 1);
    assert_eq!(placed.room_hero_datas[0].hero_id, Some(3023));
    let next_refresh_time = placed.room_hero_datas[0].next_refresh_time.unwrap();
    let refresh_delay = next_refresh_time - common::time::ServerTime::now_sec_i32();
    assert!((599..=600).contains(&refresh_delay));
    assert!(
        rooms
            .update_room_hero_data(&pool, config::configs::get(), &[999_999])
            .await
            .is_err()
    );

    sqlx::query(
        "UPDATE user_room_heroes SET current_faith = 50, current_minute = 1200
             WHERE user_id = 13 AND hero_id = 3023",
    )
    .execute(&pool)
    .await
    .unwrap();

    UserHeroModel::new(13, pool.clone())
        .create_hero(3022)
        .await
        .unwrap();
    let expanded = rooms
        .update_room_hero_data(&pool, config::configs::get(), &[3023, 3022])
        .await
        .unwrap();
    let retained = expanded
        .room_hero_datas
        .iter()
        .find(|hero| hero.hero_id == Some(3023))
        .unwrap();
    assert_eq!(retained.current_faith, Some(50));
    assert_eq!(retained.current_minute, Some(1200));
    assert_eq!(retained.next_refresh_time, Some(next_refresh_time));
    let added = expanded
        .room_hero_datas
        .iter()
        .find(|hero| hero.hero_id == Some(3022))
        .unwrap();
    assert_eq!(added.skin, Some(302201));
    assert_eq!(added.current_faith, Some(0));

    let gain = rooms
        .gain_room_hero_faith(&pool, config::configs::get(), &[3023])
        .await
        .unwrap();
    assert_eq!(
        gain.material_changes,
        vec![(reward::RewardMaterialType::Faith.id(), 3023, 50)]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT faith FROM heroes WHERE user_id = 13 AND hero_id = 3023"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        50
    );
    assert_eq!(gain.reply.room_hero_datas[0].current_faith, Some(0));
    assert_eq!(
        gain.reply.room_hero_datas[0].next_refresh_time,
        Some(next_refresh_time)
    );

    let removed = rooms
        .update_room_hero_data(&pool, config::configs::get(), &[3022])
        .await
        .unwrap();
    assert_eq!(removed.room_hero_datas.len(), 1);
    assert_eq!(removed.room_hero_datas[0].hero_id, Some(3022));

    let cleared = rooms
        .update_room_hero_data(&pool, config::configs::get(), &[])
        .await
        .unwrap();
    assert!(cleared.room_hero_datas.is_empty());
}

#[tokio::test]
async fn room_tasks_follow_confirmed_room_state() {
    let pool = room_test_pool(14, "room-tasks").await;
    database::db::starter_data::load_all_starter_data(&pool, 14)
        .await
        .unwrap();

    block_packages::update_block_package(&pool, 14, 6, &[], &[101, 102, 103, 104, 105])
        .await
        .unwrap();
    for block_id in 101..=105 {
        block_packages::save_block(
            &pool,
            &StoredBlockInfo {
                user_id: 14,
                block_id,
                x: block_id,
                y: 0,
                rotate: 0,
                water_type: -1,
                block_color: -1,
            },
        )
        .await
        .unwrap();
    }
    let building = buildings::create_building(&pool, 14, 5002).await.unwrap();
    buildings::use_building(&pool, 14, building.uid, 0, 0, 0)
        .await
        .unwrap();

    RoomManager::new(14)
        .sync_room_tasks(&pool, config::configs::get())
        .await
        .unwrap();
    let tasks = task_db::list_by_types(&pool, 14, vec![task_db::TaskType::Room.id()])
        .await
        .unwrap();
    let task = |id| tasks.iter().find(|task| task.task_id == id).unwrap();
    assert_eq!((task(60001).progress, task(60001).has_finished), (5, true));
    assert_eq!((task(60002).progress, task(60002).has_finished), (1, true));
    assert_eq!(task(60007).progress, 108);
}

#[test]
fn formula_materials_scale_static_format() {
    let costs = scaled_formula_materials("1#110101#3|1#110201#4", 2);

    assert_eq!(costs.items, vec![(110101, 6), (110201, 8)]);
    assert!(costs.currencies.is_empty());
}

#[test]
fn formula_produce_scales_currency_static_format() {
    let produce = scaled_formula_materials("2#5#30", 3);

    assert_eq!(produce.currencies, vec![(5, 90)]);
    assert_eq!(produce.material_changes(), vec![(2, 5, 90)]);
}

#[test]
fn production_vigor_bonus_scales_and_combines_live_formula_output() {
    let mut produce = reward::RewardSet::default();
    produce.extend(scaled_formula_materials("2#5#35", 72));
    produce.extend(scaled_formula_materials("2#5#35", 60));
    produce.extend(scaled_formula_materials("2#5#35", 60));

    scale_production_rewards(&mut produce, 1_500);

    assert_eq!(produce.material_changes(), vec![(2, 5, 10_080)]);
}

#[test]
fn room_theme_bonus_static_format_is_room_building_reward() {
    let bonus = reward::parse("11#5012#1");

    assert_eq!(bonus.room_buildings, vec![(5012, 1)]);
    assert!(bonus.items.is_empty());
}

#[test]
fn permanent_block_info_uses_room_block_color_config() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let infos =
        permanent_infos_for_blocks(config::configs::get(), &BTreeSet::from([-20000, 12345]));

    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].block_id, Some(-20000));
    assert_eq!(infos[0].color, Some(10000));
}

#[tokio::test]
async fn switching_used_plan_replaces_room_and_preserves_previous_layout() {
    let pool = room_test_pool(7, "room").await;
    block_packages::add_block_package(&pool, 7, 6)
        .await
        .unwrap();
    block_packages::update_block_package(&pool, 7, 6, &[101], &[202])
        .await
        .unwrap();
    block_packages::save_block(
        &pool,
        &StoredBlockInfo {
            user_id: 7,
            block_id: 202,
            x: 2,
            y: 0,
            rotate: 0,
            water_type: -1,
            block_color: -1,
        },
    )
    .await
    .unwrap();
    room_plan::save_room_plan(
        &pool,
        7,
        &RoomPlanInfo {
            id: Some(3),
            infos: vec![sonettobuf::BlockInfo {
                block_id: Some(101),
                x: Some(1),
                y: Some(0),
                rotate: Some(0),
                water_type: Some(-1),
                block_color: Some(-1),
            }],
            name: Some("Saved".into()),
            cover_id: Some(1),
            block_count: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    RoomManager::new(7)
        .switch_room_plan(&pool, config::configs::get(), 0, 3)
        .await
        .unwrap();

    let placed = block_packages::get_blocks(&pool, 7).await.unwrap();
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].block_id, 101);
    let active = room_plan::get_room_plan(&pool, 7, 0)
        .await
        .unwrap()
        .unwrap();
    let saved = room_plan::get_room_plan(&pool, 7, 3)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.infos[0].block_id, Some(101));
    assert_eq!(saved.infos[0].block_id, Some(202));
    let package: sonettobuf::BlockPackageInfo = block_packages::get_block_packages(&pool, 7)
        .await
        .unwrap()
        .pop()
        .unwrap()
        .into();
    assert_eq!(package.use_block_ids, vec![101]);
    assert_eq!(package.un_use_block_ids, vec![202]);
}

#[tokio::test]
async fn generated_roads_receive_ids_and_follow_room_revert() {
    let pool = room_test_pool(8, "roads").await;

    let rooms = RoomManager::new(8);
    let generated = rooms
        .generate_roads(
            &pool,
            Vec::new(),
            vec![RoadInfo {
                id: Some(0),
                from_type: Some(1),
                to_type: Some(2),
                road_points: vec![sonettobuf::RoadPoint {
                    x: Some(3),
                    y: Some(4),
                }],
                ..Default::default()
            }],
        )
        .await
        .unwrap();
    assert_eq!(generated.valid_road_infos[0].id, Some(1));
    rooms.room_confirm(&pool).await.unwrap();

    rooms.delete_roads(&pool, vec![1]).await.unwrap();
    assert!(
        block_packages::get_roads(&pool, 8)
            .await
            .unwrap()
            .is_empty()
    );
    rooms.room_revert(&pool).await.unwrap();

    let restored = block_packages::get_roads(&pool, 8).await.unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, 1);
}

#[tokio::test]
async fn character_interaction_uses_config_and_rewards_only_once() {
    let pool = room_test_pool(9, "interaction").await;
    sqlx::query("INSERT INTO user_room_heroes (user_id, hero_id) VALUES (9, 3015)")
        .execute(&pool)
        .await
        .unwrap();

    let rooms = RoomManager::new(9);
    let started = rooms
        .start_character_interaction(&pool, config::configs::get(), 1_301_501)
        .await
        .unwrap();
    assert_eq!(started.id, Some(1_301_501));

    let completed = rooms
        .complete_character_interaction(&pool, config::configs::get(), 1_301_501, vec![1_000_101])
        .await
        .unwrap();
    assert_eq!(completed.reply.id, Some(1_301_501));
    assert_eq!(completed.reply.select_ids, vec![1_000_101]);
    assert_eq!(
        character_interactions::get_interaction_count(&pool, 9)
            .await
            .unwrap(),
        1
    );
    assert!(
        rooms
            .complete_character_interaction(
                &pool,
                config::configs::get(),
                1_301_501,
                vec![1_000_101],
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn copying_another_current_plan_applies_the_copied_layout() {
    let pool = room_test_pool(10, "owner").await;
    player_infos::create_player_info(&pool, 10, 0)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (11, 'visitor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for user_id in [10, 11] {
        block_packages::add_block_package(&pool, user_id, 6)
            .await
            .unwrap();
        block_packages::update_block_package(&pool, user_id, 6, &[101], &[])
            .await
            .unwrap();
    }
    block_packages::save_block(
        &pool,
        &StoredBlockInfo {
            user_id: 10,
            block_id: 101,
            x: 4,
            y: 5,
            rotate: 1,
            water_type: -1,
            block_color: -1,
        },
    )
    .await
    .unwrap();

    let copied = RoomManager::new(11)
        .copy_other_room_plan(&pool, config::configs::get(), 10, 0, 2, "Copied".into())
        .await
        .unwrap();

    assert_eq!(copied.id, Some(0));
    let blocks = block_packages::get_blocks(&pool, 11).await.unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!((blocks[0].block_id, blocks[0].x, blocks[0].y), (101, 4, 5));

    RoomManager::new(10)
        .generate_roads(
            &pool,
            Vec::new(),
            vec![RoadInfo {
                road_points: vec![sonettobuf::RoadPoint {
                    x: Some(1),
                    y: Some(2),
                }],
                ..Default::default()
            }],
        )
        .await
        .unwrap();
    let rooms = RoomManager::new(10);
    let limits = rooms
        .room_plan_info(&pool, config::configs::get())
        .await
        .unwrap();
    assert_eq!(
        (limits.can_share_count, limits.can_use_share_count),
        (Some(10), Some(30))
    );
    let share = rooms.share_room_plan(&pool, 0).await.unwrap();
    assert_eq!(share.can_share_count, Some(9));
    let share_code = share.share_code.unwrap();
    let shared = rooms
        .get_room_share(&pool, share_code.clone())
        .await
        .unwrap();
    assert_eq!(shared.nick_name.as_deref(), Some("owner"));
    assert_eq!(shared.road_infos.len(), 1);
    assert_eq!(
        RoomManager::new(11)
            .other_room_ob_info(&pool, 10)
            .await
            .unwrap()
            .share_code,
        Some(share_code)
    );
}

#[tokio::test]
async fn episode_condition_uses_saved_dungeon_progress() {
    let pool = room_test_pool(12, "condition").await;
    for hero_id in [3086, 3061] {
        sqlx::query("INSERT INTO user_room_heroes (user_id, hero_id) VALUES (12, ?)")
            .bind(hero_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert!(
        RoomManager::new(12)
            .start_character_interaction(&pool, config::configs::get(), 109_308_621)
            .await
            .is_err()
    );

    sqlx::query(
        "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, created_at, updated_at)
             VALUES (12, 107, 10730, 1, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let started = RoomManager::new(12)
        .start_character_interaction(&pool, config::configs::get(), 109_308_621)
        .await
        .unwrap();
    assert_eq!(started.id, Some(109_308_621));
}

#[tokio::test]
async fn room_reddot_commands_hide_owned_entries() {
    let pool = room_test_pool(13, "reddots").await;
    block_packages::add_block_package(&pool, 13, 6)
        .await
        .unwrap();
    buildings::create_building(&pool, 13, 5001).await.unwrap();
    for (define_id, info_id) in [
        (RedDotId::RoomBlockPackage.id(), 6),
        (RedDotId::RoomBuildingPlace.id(), 5001),
    ] {
        red_dots::upsert_red_dot(&pool, 13, define_id, info_id, 1, 0, String::new(), false)
            .await
            .unwrap();
    }

    let rooms = RoomManager::new(13);
    rooms.hide_block_package_reddot(&pool, 6).await.unwrap();
    rooms.hide_building_reddot(&pool, 5001).await.unwrap();

    let dots = red_dots::get_red_dots_by_defines(
        &pool,
        13,
        vec![
            RedDotId::RoomBlockPackage.id(),
            RedDotId::RoomBuildingPlace.id(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(dots.len(), 2);
    assert!(dots.iter().all(|dot| dot.value == 0));
}
