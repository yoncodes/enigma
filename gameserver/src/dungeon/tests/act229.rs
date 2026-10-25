use super::*;

#[tokio::test]
async fn act229_victory_uses_only_act229_settlement() {
    use crate::{
        handlers::dungeon::on_fight_end_fight,
        net::{
            app::AppState, context::ConnectionContext, outbound::CommandPacket,
            packet::ClientPacket,
        },
        player::{Player, PlayerState, battle::Act229BattleContext},
    };
    use prost::Message;
    use sonettobuf::{CmdId, EndFightRequest, Fight, FightEntityInfo, FightTeam};
    use tokio::sync::mpsc;

    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (35, 'act229-settlement', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let runtime = runtime(Fight {
        cur_round: Some(2),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let active = ActiveBattle {
        runtime,
        act229_context: Some(Act229BattleContext {
            activity_id: 138521,
            stage_id: 1,
        }),
        ..Default::default()
    };

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(35, PlayerState::new(35, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);

    let mut data = Vec::new();
    EndFightRequest {
        is_abort: Some(false),
    }
    .encode(&mut data)
    .unwrap();
    on_fight_end_fight(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::FightEndFightCmd as i16,
            up_tag: 7,
            data,
        },
    )
    .await
    .unwrap();

    let commands = std::iter::from_fn(|| packets.try_recv().ok())
        .map(|packet| match packet {
            CommandPacket::Push { cmd_id, .. } | CommandPacket::Reply { cmd_id, .. } => cmd_id,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            CmdId::Act229BattleFinishPushCmd,
            CmdId::FightEndFightPushCmd,
            CmdId::FightEndFightCmd,
        ]
    );
    let star: i32 = sqlx::query_scalar(
        "SELECT star FROM user_activity229_stages
         WHERE user_id = 35 AND activity_id = 138521 AND stage_id = 1",
    )
    .fetch_one(ctx.state.db)
    .await
    .unwrap();
    assert_eq!(star, 1);
}

#[tokio::test]
async fn act229_star_errors_leave_the_fight_active() {
    use crate::{
        handlers::dungeon::on_fight_end_fight,
        net::{app::AppState, context::ConnectionContext, packet::ClientPacket},
        player::{Player, PlayerState, battle::Act229BattleContext},
    };
    use prost::Message;
    use sonettobuf::{CmdId, EndFightRequest, Fight, FightEntityInfo, FightTeam};
    use tokio::sync::mpsc;

    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (36, 'act229-star-error', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 36,
            episode_id: 0,
            battle_id: i32::MAX,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let active = ActiveBattle {
        fight_id: Some(fight_id),
        battle_id: i32::MAX,
        runtime: runtime(Fight {
            cur_round: Some(2),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    current_hp: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        act229_context: Some(Act229BattleContext {
            activity_id: 138521,
            stage_id: 1,
        }),
        ..Default::default()
    };
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, _) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(36, PlayerState::new(36, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);
    let mut data = Vec::new();
    EndFightRequest {
        is_abort: Some(false),
    }
    .encode(&mut data)
    .unwrap();

    assert!(
        on_fight_end_fight(
            &mut ctx,
            ClientPacket {
                sequence: 0,
                cmd_id: CmdId::FightEndFightCmd as i16,
                up_tag: 7,
                data,
            },
        )
        .await
        .is_err()
    );
    assert!(ctx.player().unwrap().battle.active_snapshot().is_some());
    assert!(
        battle_db::load_active_fight(ctx.state.db, 36)
            .await
            .unwrap()
            .is_some()
    );
    let stages: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_activity229_stages WHERE user_id = 36")
            .fetch_one(ctx.state.db)
            .await
            .unwrap();
    assert_eq!(stages, 0);
}

#[test]
fn episode_first_bonus_uses_bonus_config() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());

    assert_eq!(reward::parse_bonus(1010902).player_cloths, vec![(1, 1)]);
}

#[test]
fn episode_exp_uses_cost_and_player_level_thresholds() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let episode = config::configs::get().episode.get(10101).unwrap();

    let rewards = logic::dungeon::completion_rewards(episode, false, 1, 1, 2);
    assert!(rewards.normal_bonus.contains(&(3, 0, 160)));
    assert!(rewards.normal_bonus.contains(&(2, 3, 160)));
    assert_eq!(episode_cost(episode, 2).currencies, vec![(4, 16)]);
    assert_eq!(failure_refund(episode, 2).currencies, vec![(4, 16)]);
    assert_eq!(database::db::game::player_infos::level_for_exp(199), 1);
    assert_eq!(database::db::game::player_infos::level_for_exp(200), 2);
    let level_rewards = crate::logic::profile::level_up_rewards(
        database::db::game::player_infos::PlayerLevelChange { from: 1, to: 2 },
    );
    assert_eq!(level_rewards.currencies, vec![(3, 1), (4, 50)]);
}

#[test]
fn dungeon_record_auto_saves_only_new_or_faster_runs() {
    assert!(should_save_record(None, 3));
    assert!(should_save_record(Some(4), 3));
    assert!(should_save_record(Some(3), 3));
    assert!(!should_save_record(Some(2), 3));
}

#[test]
fn battle_star_uses_configured_round_condition() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let runtime = runtime(sonettobuf::Fight {
        cur_round: Some(3),
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(battle_star(&runtime, 301110).unwrap(), 2);
    assert_eq!(battle_star(&runtime, 70202).unwrap(), 2);
}

#[tokio::test]
async fn puzzle_progress_belongs_to_an_unlocked_element_and_clears_on_finish() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (13, 'puzzle', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
             VALUES (13, 101, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        save_puzzle_progress(&pool, 13, 999, "x".into())
            .await
            .is_err()
    );
    assert_eq!(
        get_puzzle_progress(&pool, 13, 101)
            .await
            .unwrap()
            .progress
            .as_deref(),
        Some("")
    );
    save_puzzle_progress(&pool, 13, 101, "path".into())
        .await
        .unwrap();
    assert_eq!(
        get_puzzle_progress(&pool, 13, 101)
            .await
            .unwrap()
            .progress
            .as_deref(),
        Some("path")
    );
    finish_puzzle(&pool, 13, 101).await.unwrap();
    let saved: String = sqlx::query_scalar(
        "SELECT puzzle_progress FROM user_dungeon_elements
             WHERE user_id = 13 AND element_id = 101",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(saved.is_empty());
    assert_eq!(
        dungeons::get_finished_puzzles(&pool, 13).await.unwrap(),
        vec![101]
    );
}

#[tokio::test]
async fn assist_roster_returns_the_requested_hero_from_other_accounts() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    for (id, name) in [(1_i64, "owner"), (2, "helper")] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        database::db::starter_data::load_all_starter_data(&pool, id)
            .await
            .unwrap();
    }
    database::models::game::heros::UserHeroModel::new(2, pool.clone())
        .create_hero(3023)
        .await
        .unwrap();
    database::db::game::friends::add_friend(&pool, 1, 2)
        .await
        .unwrap();

    let reply = refresh_assist(
        &pool,
        1,
        RefreshAssistRequest {
            assist_type: Some(6),
            ext: Some("3023".into()),
        },
    )
    .await
    .unwrap();

    let candidate = &reply.assist_hero_careers[0].assist_hero_infos[0];
    assert_eq!(candidate.user_id, Some(2));
    assert_eq!(candidate.hero_id, Some(3023));
    assert_eq!(candidate.is_friend, Some(true));
}

#[tokio::test]
async fn settlement_commits_all_or_rolls_back_with_the_active_fight() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (23, 'settlement', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 23)
        .await
        .unwrap();
    database::models::game::heros::UserHeroModel::new(23, pool.clone())
        .create_hero(3023)
        .await
        .unwrap();

    let episode = configs::get().episode.get(10101).unwrap();
    let chapter_id = episode.chapter_id;
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 23,
            episode_id: episode.id,
            battle_id: episode.battle_id,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let exp_before: i32 = sqlx::query_scalar("SELECT exp FROM users WHERE id = 23")
        .fetch_one(&pool)
        .await
        .unwrap();
    let hero_uid: i64 = sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = 23 LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut active = ActiveBattle {
        fight_id: Some(fight_id + 1),
        chapter_id,
        episode_id: episode.id,
        battle_id: episode.battle_id,
        fight_group: Some(sonettobuf::FightGroup {
            hero_list: vec![hero_uid],
            ..Default::default()
        }),
        seed: u64::MAX - 7,
        ..Default::default()
    };
    let record = prepare_dungeon_record(&pool, 23, &active, 3).await.unwrap();
    assert_eq!(
        dungeons::dungeon_record_round(&pool, 23, episode.id)
            .await
            .unwrap(),
        None
    );
    let result = settle_active(
        &pool,
        23,
        &active,
        DungeonCompletion {
            star: 1,
            total_round: 3,
            multiplier: 1,
            fight_group: active.fight_group.as_ref(),
        },
        &record,
    )
    .await;

    assert!(result.is_err());
    assert_eq!(
        dungeons::episode_star(&pool, 23, episode.id).await.unwrap(),
        0
    );
    let exp_after: i32 = sqlx::query_scalar("SELECT exp FROM users WHERE id = 23")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(exp_after, exp_before);
    assert_eq!(
        dungeons::dungeon_record_round(&pool, 23, episode.id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        battle_db::load_active_fight(&pool, 23)
            .await
            .unwrap()
            .unwrap()
            .id,
        fight_id
    );

    active.fight_id = Some(fight_id);
    let settlement = settle_active(
        &pool,
        23,
        &active,
        DungeonCompletion {
            star: 1,
            total_round: 3,
            multiplier: 1,
            fight_group: active.fight_group.as_ref(),
        },
        &record,
    )
    .await
    .unwrap();
    assert_eq!(settlement.end_dungeon.update_dungeon_record, Some(true));
    assert_eq!(
        settlement
            .dungeon_update
            .dungeon_info
            .as_ref()
            .and_then(|dungeon| dungeon.has_record),
        Some(true)
    );

    assert_eq!(
        dungeons::episode_star(&pool, 23, episode.id).await.unwrap(),
        1
    );
    let committed_exp: i32 = sqlx::query_scalar("SELECT exp FROM users WHERE id = 23")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(committed_exp > exp_before);
    assert_eq!(
        dungeons::dungeon_record_round(&pool, 23, episode.id)
            .await
            .unwrap(),
        Some(3)
    );
    assert_eq!(
        dungeons::load_dungeon_record_seed(&pool, 23, episode.id)
            .await
            .unwrap(),
        Some(active.seed)
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT has_record FROM user_dungeons WHERE user_id = 23 AND episode_id = ?"
        )
        .bind(episode.id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert!(
        battle_db::load_active_fight(&pool, 23)
            .await
            .unwrap()
            .is_none()
    );
}
