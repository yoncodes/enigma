use super::*;
use std::collections::HashMap;

use sonettobuf::{
    HeroStoryPlotFinishReply, HeroStoryPlotFinishRequest, UnlocHeroStoryReply,
    UnlocHeroStoryRequest, UpdateAchievementPush,
};

#[tokio::test]
async fn hero_story_unlock_routes_cost_push_before_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 919;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'hero-story-unlock-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies
         (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, 24, 1, 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    UnlocHeroStoryRequest { story_id: Some(1) }
        .encode(&mut data)
        .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::UnlocHeroStoryCmd as i16,
        up_tag: 12,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Hero story unlock did not emit a currency snapshot push");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency[0].currency_id, Some(24));
    assert_eq!(currency.change_currency[0].quantity, Some(0));

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Hero story unlock did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::UnlocHeroStoryCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 12);
    let info = UnlocHeroStoryReply::decode(&*body).unwrap().info.unwrap();
    assert_eq!(info.story_id, Some(1));
    assert_eq!(info.unlock, Some(true));
    assert!(packets.try_recv().is_err());
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT unlock FROM user_hero_story_state WHERE user_id = ? AND story_id = 1",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn hero_story_plot_finish_routes_task_update_before_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 922;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'hero-story-plot-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        logic::story::StoryManager::new(player_id)
            .finish_hero_story_plot(&pool, 303707, configs::get())
            .await
            .is_err()
    );
    sqlx::query(
        "INSERT INTO user_hero_story_state (user_id, story_id, unlock, updated_at)
         VALUES (?, 26, 1, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_necrologist_story_plots (user_id, story_id, plot_id, state)
         VALUES (?, 26, 303701, 4)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    HeroStoryPlotFinishRequest { plot: Some(303707) }
        .encode(&mut data)
        .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::HeroStoryPlotFinishCmd as i16,
        up_tag: 13,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request.clone()).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Hero story plot finish did not emit a task update");
    };
    assert_eq!(cmd_id, CmdId::UpdateTaskPushCmd);
    let tasks = UpdateTaskPush::decode(&*body)
        .unwrap()
        .task_info
        .into_iter()
        .map(|task| (task.id, task.progress))
        .collect::<HashMap<_, _>>();
    assert_eq!(tasks.get(&650001), Some(&1));
    assert_eq!(tasks.get(&650007), Some(&1));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Hero story plot finish did not emit an achievement update");
    };
    assert_eq!(cmd_id, CmdId::UpdateAchievementPushCmd);
    let achievement = UpdateAchievementPush::decode(&*body).unwrap();
    assert_eq!(achievement.infos[0].id, Some(41250701));
    assert_eq!(achievement.infos[0].progress, Some(1));

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Hero story plot finish did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::HeroStoryPlotFinishCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 13);
    assert_eq!(
        HeroStoryPlotFinishReply::decode(&*body).unwrap().plot,
        Some(303707)
    );
    assert!(packets.try_recv().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_tasks
             WHERE user_id = ? AND type_id = 65 AND task_id = 650007",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        1
    );

    dispatch_command(&mut ctx, request).await.unwrap();
    let CommandPacket::Reply {
        cmd_id,
        result_code,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Replayed hero story plot finish did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::HeroStoryPlotFinishCmd);
    assert_eq!(result_code, 0);
    assert!(packets.try_recv().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_tasks
             WHERE user_id = ? AND type_id = 65 AND task_id = 650007",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_achievements
             WHERE user_id = ? AND achievement_id = 41250701",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        1
    );
}
