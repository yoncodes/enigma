use super::*;
use crate::{handlers::dungeon::{on_dungeon_end_dungeon, on_fight_end_fight}, net::{app::AppState,
    context::ConnectionContext, outbound::CommandPacket, packet::ClientPacket}, player::{Player, PlayerState}};
use database::db::game::battle::{create_fight_instance, NewFightInstance};
use prost::Message;
use sonettobuf::{CmdId, EndDungeonPush, EndDungeonRequest, EndFightRequest, Fight, FightEntityInfo,
    FightTeam, HeroAttribute, TowerBattleFinishPush};
use tokio::sync::mpsc;

async fn active(db: &SqlitePool, user: i64, layer: i32) -> ActiveBattle {
    let cfg = configs::get().tower_boss_episode.iter().find(|row| row.tower_id == 6 && row.layer_id == layer).unwrap();
    let ep = configs::get().episode.get(cfg.episode_id).unwrap();
    let fight_id = create_fight_instance(db, NewFightInstance { user_id: user, episode_id: ep.id,
        battle_id: ep.battle_id, multiplication: 1, entry_cost: "{}", checkpoint: "{}", created_at: 0 }).await.unwrap();
    let entity = |uid, hp, side| FightEntityInfo { uid: Some(uid), team_type: Some(side), current_hp: Some(hp),
        attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }), ..Default::default() };
    ActiveBattle { tower_type: Some(2), tower_id: Some(6), layer_id: Some(layer), episode_id: ep.id,
        chapter_id: ep.chapter_id, battle_id: ep.battle_id, fight_id: Some(fight_id), multiplication: Some(1),
        runtime: runtime(Fight { episode_id: Some(ep.id), battle_id: Some(ep.battle_id), cur_round: Some(1),
            attacker: Some(FightTeam { entitys: vec![entity(10,100,1)], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![entity(-1,0,2)], ..Default::default() }), ..Default::default() }),
        ..Default::default() }
}

#[tokio::test]
async fn isle_of_echoes_first_clear_settles_unlocks_boss_and_grants_configured_rewards_once() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user = 976001;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'isle-boss-test', 0, 0)")
        .bind(user).execute(&pool).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(64);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user, PlayerState::new(user, 0)));
    for (attempt, use_end_dungeon) in [(0, false), (1, true)] {
        let battle = active(ctx.state.db, user, 1).await;
        assert!(battle.is_victory());
        ctx.player_mut().unwrap().battle.restore_active(battle);
        if use_end_dungeon {
            on_dungeon_end_dungeon(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::DungeonEndDungeonCmd as i16,
                up_tag: 1, data: EndDungeonRequest { is_abort: Some(false), ..Default::default() }.encode_to_vec() }).await.unwrap();
        } else {
            on_fight_end_fight(&mut ctx, ClientPacket { sequence: 0, cmd_id: CmdId::FightEndFightCmd as i16,
                up_tag: 1, data: EndFightRequest { is_abort: Some(false) }.encode_to_vec() }).await.unwrap();
        }
        let mut finish = None; let mut result = None; let mut commands = Vec::new();
        while let Ok(packet) = packets.try_recv() {
            if let CommandPacket::Push { cmd_id, body, .. } = packet {
                if cmd_id == CmdId::TowerBattleFinishPushCmd { finish = Some(TowerBattleFinishPush::decode(&*body).unwrap()); }
                if cmd_id == CmdId::DungeonEndDungeonPushCmd { result = Some(EndDungeonPush::decode(&*body).unwrap()); }
                commands.push(cmd_id);
            }
        }
        let finish = finish.expect("missing boss settlement push");
        assert_eq!((finish.r#type, finish.tower_id, finish.layer_id, finish.boss_level), (Some(2), Some(6), Some(1), Some(1)));
        let result = result.expect("missing dungeon settlement push");
        assert_eq!(result.first_pass, Some(attempt == 0));
        assert_eq!(result.first_bonus.len(), if attempt == 0 { 3 } else { 0 });
        let position = |id| commands.iter().position(|value| *value == id).unwrap();
        assert!(position(CmdId::TowerBattleFinishPushCmd) < position(CmdId::FightEndFightPushCmd));
        let qty: i32 = sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = ? AND item_id = 620101")
            .bind(user).fetch_one(state.db).await.unwrap();
        assert_eq!(qty, 80);
    }
    let pass: i32 = sqlx::query_scalar("SELECT pass_layer_id FROM user_towers WHERE user_id = ? AND tower_type = 2 AND tower_id = 6")
        .bind(user).fetch_one(state.db).await.unwrap();
    let level: i32 = sqlx::query_scalar("SELECT level FROM user_assist_bosses WHERE user_id = ? AND boss_id = 6")
        .bind(user).fetch_one(state.db).await.unwrap();
    let plans: Vec<i32> = sqlx::query_scalar("SELECT talent_point FROM user_assist_boss_talent_plans WHERE user_id = ? AND boss_id = 6 ORDER BY plan_id")
        .bind(user).fetch_all(state.db).await.unwrap();
    assert_eq!((pass, level), (1, 1));
    assert_eq!(plans, vec![1,1,1,1]);
}

#[tokio::test]
async fn isle_of_echoes_rejects_unreached_layer_without_progress_or_rewards() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user = 976002;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'isle-no-skip', 0, 0)")
        .bind(user).execute(&pool).await.unwrap();
    let battle = active(&pool, user, 2).await;
    assert!(crate::tower::settle_boss_victory(&pool, user, &battle).await.is_err());
    let fights: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE user_id = ? AND active = 1")
        .bind(user).fetch_one(&pool).await.unwrap();
    let grants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE user_id = ?")
        .bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!((fights, grants), (1,0));
}

#[tokio::test]
async fn isle_of_echoes_tutorial_records_completion_and_awards_once_without_unlocking_boss() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    let user = 976004;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'isle-teach-test', 0, 0)")
        .bind(user).execute(&db).await.unwrap();
    let teach = configs::get().tower_boss_teach.iter().find(|row| row.tower_id == 6 && row.teach_id == 601).unwrap();
    let ep = configs::get().episode.get(teach.episode_id).unwrap();
    for attempt in 0..2 {
        let fight_id = create_fight_instance(&db, NewFightInstance { user_id: user, episode_id: ep.id,
            battle_id: ep.battle_id, multiplication: 1, entry_cost: "{}", checkpoint: "{}", created_at: 0 }).await.unwrap();
        let entity = |uid, hp, side| FightEntityInfo { uid: Some(uid), current_hp: Some(hp), team_type: Some(side),
            attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }), ..Default::default() };
        let battle = ActiveBattle { tower_type: Some(2), tower_id: Some(6), layer_id: Some(0), difficulty: Some(601),
            episode_id: ep.id, chapter_id: ep.chapter_id, battle_id: ep.battle_id, fight_id: Some(fight_id),
            runtime: runtime(Fight { episode_id: Some(ep.id), battle_id: Some(ep.battle_id), cur_round: Some(1),
                attacker: Some(FightTeam { entitys: vec![entity(1,100,1)], ..Default::default() }),
                defender: Some(FightTeam { entitys: vec![entity(-1,0,2)], ..Default::default() }), ..Default::default() }),
            ..Default::default() };
        assert!(battle.is_victory());
        let result = crate::tower::settle_boss_victory(&db, user, &battle).await.unwrap();
        assert_eq!(result.end_dungeon.first_pass, Some(attempt == 0));
        assert_eq!(result.finish.layer_id, Some(0));
        assert_eq!(result.end_dungeon.first_bonus.len(), if attempt == 0 { 2 } else { 0 });
    }
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tower_pass_teaches WHERE user_id = ? AND tower_type = 2 AND tower_id = 6 AND teach_id = 601")
        .bind(user).fetch_one(&db).await.unwrap();
    let bosses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_assist_bosses WHERE user_id = ? AND boss_id = 6")
        .bind(user).fetch_one(&db).await.unwrap();
    assert_eq!((stored, bosses), (1,0));
}

#[tokio::test]
async fn every_echo_tutorial_records_its_completion_and_first_rewards_once() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    let user = 976005;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'all-echo-teaches', 0, 0)")
        .bind(user).execute(&db).await.unwrap();
    let teaches = configs::get().tower_boss_teach.iter().collect::<Vec<_>>();
    assert_eq!(teaches.len(), 12);
    for teach in teaches {
        let ep = configs::get().episode.get(teach.episode_id).unwrap();
        let expected = logic::reward::parse_strict(&teach.first_bonus).unwrap().material_changes();
        for first in [true, false] {
            let fight_id = create_fight_instance(&db, NewFightInstance { user_id: user,
                episode_id: ep.id, battle_id: ep.battle_id, multiplication: 1, entry_cost: "{}",
                checkpoint: "{}", created_at: 0 }).await.unwrap();
            let entity = |uid, hp, side| FightEntityInfo { uid: Some(uid), current_hp: Some(hp),
                team_type: Some(side), attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }),
                ..Default::default() };
            let active = ActiveBattle { tower_type: Some(2), tower_id: Some(teach.tower_id),
                layer_id: Some(0), difficulty: Some(teach.teach_id), episode_id: ep.id,
                chapter_id: ep.chapter_id, battle_id: ep.battle_id, fight_id: Some(fight_id),
                runtime: runtime(Fight { episode_id: Some(ep.id), battle_id: Some(ep.battle_id),
                    cur_round: Some(1), attacker: Some(FightTeam { entitys: vec![entity(1,100,1)],
                        ..Default::default() }), defender: Some(FightTeam { entitys: vec![entity(-1,0,2)],
                        ..Default::default() }), ..Default::default() }), ..Default::default() };
            let result = crate::tower::settle_boss_victory(&db, user, &active).await
                .unwrap_or_else(|error| panic!("teach {} settlement failed: {error}", teach.teach_id));
            assert_eq!(result.end_dungeon.first_pass, Some(first));
            assert_eq!(result.finish.difficulty, Some(teach.teach_id));
            let actual = result.end_dungeon.first_bonus.iter().map(|entry|
                (entry.materil_type.unwrap(), entry.materil_id.unwrap(), entry.quantity.unwrap())).collect::<Vec<_>>();
            assert_eq!(actual, if first { expected.clone() } else { Vec::new() }, "teach {}", teach.teach_id);
        }
    }
    let passed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tower_pass_teaches WHERE user_id = ?")
        .bind(user).fetch_one(&db).await.unwrap();
    let unlocked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_assist_bosses WHERE user_id = ?")
        .bind(user).fetch_one(&db).await.unwrap();
    assert_eq!((passed, unlocked), (12, 0));
    let info = crate::tower::info(&db, user).await.unwrap();
    assert_eq!(info.towers.iter().filter(|tower| tower.r#type == Some(2)
        && tower.pass_teach_ids.len() == 2).count(), 6);
}

#[tokio::test]
async fn mountain_ghost_both_echo_tutorials_start_via_the_real_tower_handler() {
    use sonettobuf::{FightGroup, StartDungeonRequest, StartTowerBattleReply, StartTowerBattleRequest, TrialHero};
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    for teach_id in [101, 102] {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user = i64::from(977100 + teach_id);
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user).bind(format!("echo-start-{teach_id}")).execute(&pool).await.unwrap();
        let teach = configs::get().tower_boss_teach.iter().find(|row| row.teach_id == teach_id).unwrap();
        let episode = configs::get().episode.get(teach.episode_id).unwrap();
        let battle = configs::get().battle.get(episode.battle_id).unwrap();
        let trials = battle.trial_heros.split('|').map(|entry| {
            let fields = entry.split('#').collect::<Vec<_>>();
            TrialHero { trial_id: Some(fields[0].parse().unwrap()), pos: Some(fields[2].parse().unwrap()),
                ..Default::default() }
        }).collect::<Vec<_>>();
        let request = StartTowerBattleRequest { r#type: Some(2), tower_id: Some(1), layer_id: Some(0),
            difficulty: Some(teach_id), talent_plan_id: Some(1),
            start_dungeon_request: Some(StartDungeonRequest { chapter_id: Some(episode.chapter_id),
                episode_id: Some(episode.id), multiplication: Some(1), fight_group: Some(FightGroup {
                    assist_boss_id: Some(1), trial_hero_list: trials, ..Default::default()
                }), ..Default::default() }) };
        let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
        let (outbound, mut packets) = mpsc::channel(32);
        let mut ctx = ConnectionContext::new(outbound, state);
        ctx.player = Some(Player::new(user, PlayerState::new(user, 0)));
        crate::handlers::tower::on_start_battle(&mut ctx, ClientPacket { sequence: 1,
            cmd_id: CmdId::StartTowerBattleCmd as i16, up_tag: 19, data: request.encode_to_vec() })
            .await.unwrap_or_else(|error| panic!("teach {teach_id} failed at entry: {error}"));
        let CommandPacket::Reply { cmd_id: CmdId::StartTowerBattleCmd, body, result_code: 0, .. }
            = packets.try_recv().unwrap() else { panic!("teach {teach_id} did not reply"); };
        let reply = StartTowerBattleReply::decode(&*body).unwrap();
        assert_eq!(reply.talent_plan_id, Some(teach.plan_id));
        let start = reply.start_dungeon_reply.unwrap();
        let attacker = start.fight.unwrap().attacker.unwrap();
        assert_eq!(attacker.entitys.len(), 4);
        assert_eq!(attacker.assist_boss.unwrap().model_id, Some(1));
        assert!(start.round.is_some());
        assert!(ctx.player().unwrap().battle.has_active());
    }
}
