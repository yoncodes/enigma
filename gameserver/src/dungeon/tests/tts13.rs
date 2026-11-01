use super::*;
use crate::{
    handlers::dungeon::on_fight_end_fight,
    net::{app::AppState, context::ConnectionContext, outbound::CommandPacket, packet::ClientPacket},
    player::{Player, PlayerState},
};
use database::db::game::battle::{create_fight_instance, NewFightInstance};
use prost::Message;
use sonettobuf::{CmdId, EndDungeonPush, EndFightPush, EndFightRequest, Fight, FightEntityInfo, FightTeam, HeroAttribute};
use tokio::sync::mpsc;

#[tokio::test]
async fn tts13_victory_sends_success_and_saves_progress() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 975_013;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'tts13-test', 0, 0)")
        .bind(player_id).execute(&pool).await.unwrap();
    database::db::starter_data::load_all_starter_data(&pool, player_id).await.unwrap();
    let initial_event_currency: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3801), 0)",
    ).bind(player_id).fetch_one(&pool).await.unwrap();
    let episode_id = 3_810_113;
    let battle_id = 119_381_070;
    let fight_id = create_fight_instance(&pool, NewFightInstance {
        user_id: player_id, episode_id, battle_id, multiplication: 1,
        entry_cost: "{}", checkpoint: "", created_at: 0,
    }).await.unwrap();
    let defender = |uid, model_id| FightEntityInfo {
        uid: Some(uid), model_id: Some(model_id), team_type: Some(2), current_hp: Some(0),
        attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
        ..Default::default()
    };
    let active = ActiveBattle {
        episode_id, chapter_id: 38_101, battle_id, fight_id: Some(fight_id),
        multiplication: Some(1), is_replay: Some(false),
        runtime: runtime(Fight {
            battle_id: Some(battle_id), episode_id: Some(episode_id), cur_round: Some(4),
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(10), team_type: Some(1), current_hp: Some(100),
                attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
                ..Default::default()
            }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![
                defender(-1, 119_381_070), defender(-2, 119_381_060), defender(-3, 119_381_060),
            ], ..Default::default() }),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(active.is_victory());
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(128);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);
    let request = || ClientPacket {
        sequence: 0, cmd_id: CmdId::FightEndFightCmd as i16, up_tag: 1,
        data: EndFightRequest { is_abort: Some(false) }.encode_to_vec(),
    };
    on_fight_end_fight(&mut ctx, request()).await.unwrap();
    let mut fight_result = None;
    let mut dungeon_result = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Push { cmd_id, body, .. } = packet {
            match cmd_id {
                CmdId::FightEndFightPushCmd => fight_result = Some(EndFightPush::decode(&*body).unwrap()),
                CmdId::DungeonEndDungeonPushCmd => dungeon_result = Some(EndDungeonPush::decode(&*body).unwrap()),
                _ => {}
            }
        }
    }
    assert_eq!(fight_result.unwrap().record.unwrap().fight_result, Some(FightResult::Succ as i32));
    let dungeon = dungeon_result.expect("the completed episode must send its dungeon result");
    assert_eq!(dungeon.episode_id, Some(episode_id));
    assert_eq!(dungeon.first_pass, Some(true));
    assert_eq!(dungeon.star, Some(2));
    assert!(dungeon.normal_bonus.iter().any(|item| {
        item.materil_type == Some(2) && item.materil_id == Some(3801) && item.quantity == Some(120)
    }), "the configured event currency must be present in the clear rewards");
    assert!(dungeon.advenced_bonus.iter().any(|item| {
        item.materil_type == Some(2) && item.materil_id == Some(2) && item.quantity == Some(60)
    }), "the configured two-star reward must be present");
    let event_currency: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3801",
    ).bind(player_id).fetch_one(state.db).await.unwrap();
    assert_eq!(event_currency, initial_event_currency + 120);
    let star = dungeons::episode_star(state.db, player_id, episode_id).await.unwrap();
    assert_eq!(star, 2, "the successful result must persist episode progress");

    on_fight_end_fight(&mut ctx, request()).await.unwrap();
    assert_eq!(dungeons::episode_star(state.db, player_id, episode_id).await.unwrap(), star);
    let repeated_currency: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3801",
    ).bind(player_id).fetch_one(state.db).await.unwrap();
    assert_eq!(repeated_currency, event_currency);
    while let Ok(packet) = packets.try_recv() {
        assert!(!matches!(packet, CommandPacket::Push { cmd_id: CmdId::DungeonEndDungeonPushCmd, .. }),
            "a repeated finish request must not settle the episode twice");
    }
}

#[tokio::test]
async fn temporary_scale_battles_credit_repeat_and_first_clear_currency() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 975_014;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'tts-rewards-test', 0, 0)")
        .bind(player_id).execute(&pool).await.unwrap();
    database::db::starter_data::load_all_starter_data(&pool, player_id).await.unwrap();

    async fn balance(db: &sqlx::SqlitePool, player: i64) -> i32 {
        sqlx::query_scalar(
            "SELECT COALESCE((SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3801), 0)",
        ).bind(player).fetch_one(db).await.unwrap()
    }
    async fn active(db: &sqlx::SqlitePool, player: i64, episode_id: i32) -> ActiveBattle {
        let episode = configs::get().episode.get(episode_id).unwrap();
        let battle_id = episode.battle_id;
        let fight_id = create_fight_instance(db, NewFightInstance {
            user_id: player, episode_id, battle_id, multiplication: 1,
            entry_cost: "{}", checkpoint: "", created_at: 0,
        }).await.unwrap();
        ActiveBattle {
            episode_id, chapter_id: episode.chapter_id, battle_id, fight_id: Some(fight_id),
            multiplication: Some(1), is_replay: Some(false),
            runtime: runtime(Fight {
                battle_id: Some(battle_id), episode_id: Some(episode_id),
                attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                    uid: Some(10), team_type: Some(1), current_hp: Some(100),
                    attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
                    ..Default::default()
                }], ..Default::default() }),
                defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                    uid: Some(-1), team_type: Some(2), current_hp: Some(0),
                    attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
                    ..Default::default()
                }], ..Default::default() }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    fn currency(materials: &[sonettobuf::MaterialData]) -> i32 {
        materials.iter().filter(|item| item.materil_type == Some(2) && item.materil_id == Some(3801))
            .map(|item| item.quantity.unwrap_or_default()).sum()
    }
    let completion = || DungeonCompletion {
        star: 2, total_round: 4, multiplier: 1, fight_group: None,
    };
    let record = DungeonRecordStatus::default();
    let mut expected = balance(&pool, player_id).await;
    for (episode_id, normal, first_bonus) in [
        (3_810_103, 120, 0), (3_830_113, 170, 340), (3_840_120, 300, 340),
    ] {
        let first = active(&pool, player_id, episode_id).await;
        let first_result = settle_active(&pool, player_id, &first, completion(), &record).await.unwrap();
        assert_eq!(first_result.end_dungeon.first_pass, Some(true));
        assert_eq!(currency(&first_result.end_dungeon.normal_bonus), normal);
        assert_eq!(currency(&first_result.end_dungeon.first_bonus), first_bonus);
        expected += normal + first_bonus;
        assert_eq!(balance(&pool, player_id).await, expected, "first clear {episode_id}");

        let second = active(&pool, player_id, episode_id).await;
        let second_result = settle_active(&pool, player_id, &second, completion(), &record).await.unwrap();
        assert_eq!(second_result.end_dungeon.first_pass, Some(false));
        assert_eq!(currency(&second_result.end_dungeon.normal_bonus), normal);
        assert_eq!(currency(&second_result.end_dungeon.first_bonus), 0);
        expected += normal;
        assert_eq!(balance(&pool, player_id).await, expected, "repeat clear {episode_id}");
    }
    let failed = active(&pool, player_id, 3_810_104).await;
    settle_failed(&pool, player_id, &failed, false).await.unwrap();
    assert_eq!(balance(&pool, player_id).await, expected, "defeat must not pay the clear reward");
    assert_eq!(dungeons::episode_star(&pool, player_id, 3_810_104).await.unwrap(), 0);
}
