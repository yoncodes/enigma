use super::*;
use crate::{
    handlers::dungeon::{on_dungeon_end_dungeon, on_fight_end_fight},
    net::{app::AppState, context::ConnectionContext, outbound::CommandPacket, packet::ClientPacket},
    player::{Player, PlayerState},
    tower,
};
use database::db::game::battle::{create_fight_instance, NewFightInstance};
use prost::Message;
use sonettobuf::{CmdId, EndDungeonPush, EndDungeonRequest, EndFightRequest, Fight, FightEntityInfo,
    FightEquip, FightGroup, FightTeam, TowerBattleFinishPush, TrialHero};
use tokio::sync::mpsc;

async fn isolated_pool(player: i64) -> SqlitePool {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(player).bind(format!("isolated-tower-{player}")).execute(&pool).await.unwrap();
    pool
}

async fn active_battle(pool: &SqlitePool, player: i64, layer: i32, episode: i32, won: bool) -> ActiveBattle {
    let battle_id = configs::get().episode.get(episode).unwrap().battle_id;
    let fight_id = create_fight_instance(pool, NewFightInstance {
        user_id: player, episode_id: episode, battle_id, multiplication: 1,
        entry_cost: "{}", checkpoint: "", created_at: 0,
    }).await.unwrap();
    ActiveBattle {
        tower_type: Some(1), tower_id: Some(0), layer_id: Some(layer),
        episode_id: episode, chapter_id: 9000, battle_id, fight_id: Some(fight_id),
        difficulty: Some(0), team_level: Some(180), assist_boss_level: Some(10),
        fight_group: Some(FightGroup { hero_list: vec![1],
            equips: vec![FightEquip { hero_uid: Some(1), equip_uid: vec![7001], ..Default::default() }],
            assist_boss_id: Some(3), ..Default::default() }),
        runtime: runtime(Fight {
            cur_round: Some(2),
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(1), model_id: Some(3120),
                current_hp: Some(if won { 100 } else { 0 }), ..Default::default() }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1),
                current_hp: Some(if won { 0 } else { 100 }), ..Default::default() }], ..Default::default() }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

async fn item_count(pool: &SqlitePool, player: i64, item: i32) -> i32 {
    sqlx::query_scalar("SELECT COALESCE((SELECT quantity FROM items WHERE user_id = ? AND item_id = ?), 0)")
        .bind(player).bind(item).fetch_one(pool).await.unwrap()
}

async fn passed_layer(pool: &SqlitePool, player: i64) -> i32 {
    sqlx::query_scalar("SELECT COALESCE((SELECT pass_layer_id FROM user_towers \
        WHERE user_id = ? AND tower_type = 1 AND tower_id = 0), 0)")
        .bind(player).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn permanent_tower_first_victory_sends_result_and_awards_once() {
    let player = 970001;
    let pool = isolated_pool(player).await;
    let active = active_battle(&pool, player, 1, 90000101, true).await;
    assert!(active.is_victory());
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player, PlayerState::new(player, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);
    on_fight_end_fight(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::FightEndFightCmd as i16,
        up_tag: 7, data: EndFightRequest { is_abort: Some(false) }.encode_to_vec() }).await.unwrap();
    let mut commands = Vec::new();
    let mut tower_result = None;
    let mut dungeon_result = None;
    while let Ok(packet) = packets.try_recv() {
        match packet {
            CommandPacket::Push { cmd_id, body, .. } => {
                if cmd_id == CmdId::TowerBattleFinishPushCmd { tower_result = Some(TowerBattleFinishPush::decode(&*body).unwrap()); }
                if cmd_id == CmdId::DungeonEndDungeonPushCmd { dungeon_result = Some(EndDungeonPush::decode(&*body).unwrap()); }
                commands.push(cmd_id);
            }
            CommandPacket::Reply { cmd_id, result_code, .. } => { assert_eq!(result_code, 0); commands.push(cmd_id); }
        }
    }
    let finish = tower_result.expect("missing tower result");
    assert_eq!((finish.r#type, finish.layer_id, finish.team_level, finish.boss_level), (Some(1), Some(1), Some(180), Some(10)));
    let episode = &finish.layer.unwrap().episode_n_os[0];
    assert_eq!((episode.episode_id, episode.status, episode.assist_boss_id), (Some(90000101), Some(1), Some(3)));
    assert_eq!((episode.heros[0].hero_id, episode.heros[0].equip_uid.as_slice()), (Some(3120), &[7001][..]));
    let end = dungeon_result.expect("missing dungeon reward result");
    assert_eq!((end.episode_id, end.first_pass, end.first_bonus.len()), (Some(90000101), Some(true), 3));
    let position = |cmd| commands.iter().position(|value| *value == cmd).unwrap();
    assert!(position(CmdId::TowerBattleFinishPushCmd) < position(CmdId::FightEndFightPushCmd));
    assert!(position(CmdId::DungeonEndDungeonPushCmd) < position(CmdId::FightEndFightPushCmd));
    assert!(position(CmdId::FightEndFightPushCmd) < position(CmdId::FightEndFightCmd));
    assert_eq!(passed_layer(state.db, player).await, 1);
    assert_eq!(item_count(state.db, player, 620102).await, 15);
    assert_eq!(item_count(state.db, player, 620101).await, 200);
    assert_eq!(item_count(state.db, player, 110103).await, 2);

    // A repeated network request has no active fight and must not award again.
    on_fight_end_fight(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::FightEndFightCmd as i16,
        up_tag: 8, data: EndFightRequest { is_abort: Some(false) }.encode_to_vec() }).await.unwrap();
    assert_eq!(item_count(state.db, player, 620102).await, 15);
    let replay = active_battle(state.db, player, 1, 90000101, true).await;
    let replay_result = tower::settle_permanent_victory(state.db, player, &replay).await.unwrap();
    assert_eq!(replay_result.end_dungeon.first_pass, Some(false));
    assert!(replay_result.end_dungeon.first_bonus.is_empty());
    assert_eq!(item_count(state.db, player, 620102).await, 15);
    assert!(tower::settle_permanent_victory(state.db, player, &replay).await.is_err());
    assert_eq!(item_count(state.db, player, 620102).await, 15);
}

#[tokio::test]
async fn permanent_tower_multi_episode_floor_rewards_only_after_all_wins() {
    let player = 970002;
    let pool = isolated_pool(player).await;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id, pass_layer_id) VALUES (?, 1, 0, 9)")
        .bind(player).execute(&pool).await.unwrap();
    for (index, episode) in [90001001, 90001002, 90001003].into_iter().enumerate() {
        let active = active_battle(&pool, player, 10, episode, true).await;
        let result = tower::settle_permanent_victory(&pool, player, &active).await.unwrap();
        let layer = result.finish.layer.unwrap();
        assert_eq!(layer.episode_n_os.iter().filter(|episode| episode.status == Some(1)).count(), index + 1);
        assert_eq!(result.end_dungeon.first_pass, Some(index == 2));
        assert_eq!(passed_layer(&pool, player).await, if index == 2 { 10 } else { 9 });
        assert_eq!(item_count(&pool, player, 620102).await, if index == 2 { 30 } else { 0 });
        assert_eq!(result.end_dungeon.first_bonus.is_empty(), index != 2);
    }
    let currency: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 2")
        .bind(player).fetch_one(&pool).await.unwrap();
    assert_eq!(currency, 240);
    let replay = active_battle(&pool, player, 10, 90001001, true).await;
    let result = tower::settle_permanent_victory(&pool, player, &replay).await.unwrap();
    assert!(!result.end_dungeon.first_pass.unwrap());
    assert!(result.end_dungeon.first_bonus.is_empty());
    assert_eq!(item_count(&pool, player, 620102).await, 30);
    database::db::game::tower::reset_sub_episode(&pool, player, 1, 0, 10, 90001002)
        .await.unwrap().unwrap();
    let reset_replay = active_battle(&pool, player, 10, 90001002, true).await;
    let result = tower::settle_permanent_victory(&pool, player, &reset_replay).await.unwrap();
    assert_eq!(result.end_dungeon.first_pass, Some(false));
    assert!(result.end_dungeon.first_bonus.is_empty());
    assert_eq!(item_count(&pool, player, 620102).await, 30);
}

#[tokio::test]
async fn permanent_tower_reward_error_rolls_back_progress_and_active_fight() {
    let player = 970003;
    let pool = isolated_pool(player).await;
    let active = active_battle(&pool, player, 1, 90000101, true).await;
    sqlx::query("CREATE TRIGGER fail_tower_reward BEFORE INSERT ON items BEGIN SELECT RAISE(ABORT, 'test reward failure'); END")
        .execute(&pool).await.unwrap();
    assert!(tower::settle_permanent_victory(&pool, player, &active).await.is_err());
    assert_eq!(passed_layer(&pool, player).await, 0);
    assert_eq!(item_count(&pool, player, 620102).await, 0);
    let status: i32 = sqlx::query_scalar("SELECT active FROM fight_instances WHERE id = ?")
        .bind(active.fight_id.unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(status, 1);
    let rows: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tower_episodes WHERE user_id = ?")
        .bind(player).fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0);
    let claims: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tower_permanent_reward_claims WHERE user_id = ?")
        .bind(player).fetch_one(&pool).await.unwrap();
    assert_eq!(claims, 0);
    sqlx::query("DROP TRIGGER fail_tower_reward").execute(&pool).await.unwrap();
    tower::settle_permanent_victory(&pool, player, &active).await.unwrap();
    assert_eq!(item_count(&pool, player, 620102).await, 15);
}

#[tokio::test]
async fn permanent_tower_alternate_end_request_settles_and_failure_does_not_reward() {
    let player = 970004;
    let pool = isolated_pool(player).await;
    let loss = active_battle(&pool, player, 1, 90000101, false).await;
    assert!(!loss.is_victory());
    assert!(tower::settle_permanent_victory(&pool, player, &loss).await.is_err());
    assert_eq!(passed_layer(&pool, player).await, 0);
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player, PlayerState::new(player, 0)));
    ctx.player_mut().unwrap().battle.restore_active(loss);
    on_fight_end_fight(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::FightEndFightCmd as i16,
        up_tag: 1, data: EndFightRequest { is_abort: Some(false) }.encode_to_vec() }).await.unwrap();
    while packets.try_recv().is_ok() {}
    assert_eq!(passed_layer(state.db, player).await, 0);
    assert_eq!(item_count(state.db, player, 620102).await, 0);
    let won = active_battle(state.db, player, 1, 90000101, true).await;
    ctx.player_mut().unwrap().battle.restore_active(won);
    on_dungeon_end_dungeon(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::DungeonEndDungeonCmd as i16,
        up_tag: 2, data: EndDungeonRequest { is_abort: Some(false), ..Default::default() }.encode_to_vec() }).await.unwrap();
    let commands: Vec<_> = std::iter::from_fn(|| packets.try_recv().ok()).map(|packet| match packet {
        CommandPacket::Push { cmd_id, .. } | CommandPacket::Reply { cmd_id, .. } => cmd_id,
    }).collect();
    assert!(commands.contains(&CmdId::TowerBattleFinishPushCmd));
    assert!(commands.contains(&CmdId::DungeonEndDungeonPushCmd));
    assert!(commands.contains(&CmdId::FightEndFightPushCmd));
    assert_eq!(passed_layer(state.db, player).await, 1);
    assert_eq!(item_count(state.db, player, 620102).await, 15);
}

#[tokio::test]
async fn permanent_tower_legacy_and_out_of_order_rewards_are_separate() {
    let legacy = 970005;
    let pool = isolated_pool(legacy).await;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id, pass_layer_id) VALUES (?, 1, 0, 9)")
        .bind(legacy).execute(&pool).await.unwrap();
    let replay = active_battle(&pool, legacy, 1, 90000101, true).await;
    let result = tower::settle_permanent_victory(&pool, legacy, &replay).await.unwrap();
    assert_eq!(result.end_dungeon.first_pass, Some(false));
    assert_eq!(item_count(&pool, legacy, 620102).await, 0);
    let prior_claims: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tower_permanent_reward_claims WHERE user_id = ?")
        .bind(legacy).fetch_one(&pool).await.unwrap();
    assert_eq!(prior_claims, 9);

    let fresh = 970006;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'isolated-out-of-order', 0, 0)")
        .bind(fresh).execute(&pool).await.unwrap();
    for (layer, episode) in [(2, 90000201), (1, 90000101)] {
        let active = active_battle(&pool, fresh, layer, episode, true).await;
        let result = tower::settle_permanent_victory(&pool, fresh, &active).await.unwrap();
        assert_eq!(result.end_dungeon.first_pass, Some(true));
    }
    assert_eq!(passed_layer(&pool, fresh).await, 2);
    assert_eq!(item_count(&pool, fresh, 620102).await, 30);
}

#[tokio::test]
async fn permanent_tower_high_floor_grants_equipment_and_preserves_trial_hero() {
    let player = 970007;
    let pool = isolated_pool(player).await;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id, pass_layer_id) VALUES (?, 1, 0, 39)")
        .bind(player).execute(&pool).await.unwrap();
    for (index, episode) in [90004001, 90004002, 90004003, 90004004].into_iter().enumerate() {
        let mut active = active_battle(&pool, player, 40, episode, true).await;
        if index == 0 {
            active.fight_group.as_mut().unwrap().trial_hero_list.push(TrialHero {
                trial_id: Some(1001), pos: Some(2), equip_uid: vec![7002], ..Default::default()
            });
        }
        let result = tower::settle_permanent_victory(&pool, player, &active).await.unwrap();
        if index == 0 {
            let heroes = &result.finish.layer.as_ref().unwrap().episode_n_os[0].heros;
            assert!(heroes.iter().any(|hero| hero.hero_id == Some(3003)
                && hero.trial_id == Some(1001) && hero.equip_uid == vec![7002]));
        }
        if index == 3 {
            assert!(result.end_dungeon.first_bonus.iter().any(|reward|
                reward.materil_type == Some(9) && reward.materil_id == Some(1000)
                    && reward.quantity == Some(1)));
        }
        assert_eq!(result.end_dungeon.first_pass, Some(index == 3));
    }
    let equipment: i32 = sqlx::query_scalar("SELECT COALESCE(SUM(count), 0) FROM equipment WHERE user_id = ? AND equip_id = 1000")
        .bind(player).fetch_one(&pool).await.unwrap();
    assert_eq!(equipment, 1);
    assert_eq!(passed_layer(&pool, player).await, 40);
    let replay = active_battle(&pool, player, 40, 90004004, true).await;
    let result = tower::settle_permanent_victory(&pool, player, &replay).await.unwrap();
    assert_eq!(result.end_dungeon.first_pass, Some(false));
    let equipment: i32 = sqlx::query_scalar("SELECT COALESCE(SUM(count), 0) FROM equipment WHERE user_id = ? AND equip_id = 1000")
        .bind(player).fetch_one(&pool).await.unwrap();
    assert_eq!(equipment, 1);
}

#[tokio::test]
async fn permanent_tower_upgrade_keeps_existing_progress_and_inventory() {
    use std::borrow::Cow;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("database/migrations");
    let all = sqlx::migrate::Migrator::new(migrations.as_path()).await.unwrap();
    let old = sqlx::migrate::Migrator {
        migrations: Cow::Owned(all.iter().filter(|migration| migration.version <= 96).cloned().collect()),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run(&pool).await.unwrap();
    let player = 970008;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'isolated-old-db', 0, 0)")
        .bind(player).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id, pass_layer_id) VALUES (?, 1, 0, 9)")
        .bind(player).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (?, 620102, 77)")
        .bind(player).execute(&pool).await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    assert_eq!(passed_layer(&pool, player).await, 9);
    assert_eq!(item_count(&pool, player, 620102).await, 77);
    let replay = active_battle(&pool, player, 1, 90000101, true).await;
    let result = tower::settle_permanent_victory(&pool, player, &replay).await.unwrap();
    assert_eq!(result.end_dungeon.first_pass, Some(false));
    assert_eq!(item_count(&pool, player, 620102).await, 77);
    assert_eq!(passed_layer(&pool, player).await, 9);
}
