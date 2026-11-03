use super::*;
use crate::{
    net::{app::AppState, context::ConnectionContext, outbound::CommandPacket},
    player::{Player, PlayerState},
};
use config::configs;
use prost::Message;
use sonettobuf::{CmdId, TeachingUpdateInfoPush};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

async fn test_context(player_id: i64) -> (ConnectionContext, mpsc::Receiver<CommandPacket>) {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, ?, 0, 0)",
    )
    .bind(player_id)
    .bind(format!("teaching-settlement-{player_id}"))
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, packets) = mpsc::channel(4096);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    (ctx, packets)
}

fn empty_settlement() -> dungeon::DungeonSettlement {
    dungeon::DungeonSettlement {
        kill_count: 0,
        hero_ids: Vec::new(),
        rewards: logic::reward::AppliedRewards::default(),
        dungeon_update: sonettobuf::DungeonUpdatePush::default(),
        open_infos: Vec::new(),
        end_dungeon: sonettobuf::EndDungeonPush::default(),
        compose_push: None,
        tasks: Vec::new(),
    }
}

#[tokio::test]
async fn teaching_settlement_push_precedes_dungeon_settlement_pushes() {
    let player_id = 905;
    let (mut ctx, mut packets) = test_context(player_id).await;
    let episode_id = configs::get().teaching_episode.iter().next().unwrap().id;
    let chapter_id = configs::get().episode.get(episode_id).unwrap().chapter_id;
    sqlx::query(
        "INSERT INTO user_dungeons
         (user_id, chapter_id, episode_id, star, challenge_count, has_record,
          left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
         VALUES (?, ?, ?, 1, 0, 0, 1, 0, 0, 0, 0)",
    )
    .bind(player_id)
    .bind(chapter_id)
    .bind(episode_id)
    .execute(ctx.state.db)
    .await
    .unwrap();

    send_completed_dungeon(
        &mut ctx,
        player_id,
        chapter_id,
        episode_id,
        empty_settlement(),
    )
    .await
    .unwrap();

    let mut push_ids = Vec::new();
    let mut teaching_body = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Push { cmd_id, body, .. } = packet {
            push_ids.push(cmd_id);
            if cmd_id == CmdId::TeachingUpdateInfoPushCmd {
                teaching_body = Some(body);
            }
        }
    }
    let teaching_index = push_ids
        .iter()
        .position(|cmd_id| *cmd_id == CmdId::TeachingUpdateInfoPushCmd)
        .unwrap();
    let dungeon_update_index = push_ids
        .iter()
        .position(|cmd_id| *cmd_id == CmdId::DungeonUpdatePushCmd)
        .unwrap();
    let dungeon_end_index = push_ids
        .iter()
        .position(|cmd_id| *cmd_id == CmdId::DungeonEndDungeonPushCmd)
        .unwrap();
    assert!(teaching_index < dungeon_update_index);
    assert!(teaching_index < dungeon_end_index);

    let push = TeachingUpdateInfoPush::decode(&*teaching_body.unwrap()).unwrap();
    assert_eq!(
        push.teaching_info,
        Some(
            logic::teaching::snapshot(ctx.state.db, player_id)
                .await
                .unwrap()
        )
    );
}

#[tokio::test]
async fn non_teaching_settlement_does_not_emit_teaching_update() {
    let player_id = 906;
    let (mut ctx, mut packets) = test_context(player_id).await;
    let episode = configs::get()
        .episode
        .iter()
        .find(|episode| !logic::teaching::is_teaching_episode(episode.id))
        .unwrap();

    send_completed_dungeon(
        &mut ctx,
        player_id,
        episode.chapter_id,
        episode.id,
        empty_settlement(),
    )
    .await
    .unwrap();

    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Push { cmd_id, .. } = packet {
            assert_ne!(cmd_id, CmdId::TeachingUpdateInfoPushCmd);
        }
    }
}

#[tokio::test]
async fn completed_battle_routes_defeated_enemies_to_activity233_tasks() {
    let player_id = 907;
    let (mut ctx, _packets) = test_context(player_id).await;
    let episode = configs::get()
        .episode
        .iter()
        .find(|episode| !logic::teaching::is_teaching_episode(episode.id))
        .unwrap();
    let mut settlement = empty_settlement();
    settlement.kill_count = 7;

    send_completed_dungeon(
        &mut ctx,
        player_id,
        episode.chapter_id,
        episode.id,
        settlement,
    )
    .await
    .unwrap();

    let task = database::db::game::tasks::get_by_id(ctx.state.db, player_id, 790006)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.progress, 7);
    assert_eq!(
        task.type_id,
        database::db::game::tasks::TaskType::ActBp.id()
    );
}
