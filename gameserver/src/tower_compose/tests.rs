use super::*;
use sqlx::sqlite::SqlitePoolOptions;

fn runtime(fight: sonettobuf::Fight) -> battle::Battle {
    battle::tooling::unstarted_battle(
        battle::catalog::BattleCatalog::new(config::configs::get()),
        fight,
    )
}

#[test]
fn battle_params_route_through_the_matching_compose_episode() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let episode = config::configs::get()
        .tower_compose_episode
        .iter()
        .find(|episode| episode.plane == 0)
        .unwrap();
    let active = ActiveBattle {
        episode_id: episode.episode_id,
        params: Some(
            serde_json::json!({
                "themeId": episode.theme_id,
                "layerId": episode.layer_id,
                "planeId": episode.plane,
            })
            .to_string(),
        ),
        ..Default::default()
    };

    assert_eq!(
        compose_battle(&active),
        Some(ComposeBattle {
            theme_id: episode.theme_id,
            layer_id: episode.layer_id,
            plane_id: episode.plane,
            ..Default::default()
        })
    );
}

#[test]
fn two_plane_battle_contains_client_switch_metadata() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let params = r#"{"themeId":1,"layerId":6,"planeId":1}"#;
    let mut fight = sonettobuf::Fight::default();
    prepare_fight_data(&mut fight, 90400199, Some(params)).unwrap();
    let data = fight.custom_data.iter().find(|data| data.r#type == Some(8)).unwrap();
    let data: serde_json::Value = serde_json::from_str(data.data.as_deref().unwrap()).unwrap();
    assert_eq!(data["themeId"], 1);
    assert_eq!(data["layerId"], 6);
    assert_eq!(data["planeId"], 1);
    assert_eq!(data["maxPlaneId"], 2);
}

#[tokio::test]
async fn winning_a_normal_layer_advances_compose_progress() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let episode = tables
        .tower_compose_episode
        .iter()
        .find(|episode| episode.plane == 0)
        .unwrap();
    let battle_id = tables.episode.get(episode.episode_id).unwrap().battle_id;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (18, 'compose', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let entity = |uid, hp| sonettobuf::FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        ..Default::default()
    };
    let active = ActiveBattle {
        episode_id: episode.episode_id,
        params: Some(
            serde_json::json!({
                "themeId": episode.theme_id,
                "layerId": episode.layer_id,
                "planeId": episode.plane,
            })
            .to_string(),
        ),
        runtime: runtime(sonettobuf::Fight {
            battle_id: Some(battle_id),
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![entity(1, 100)],
                ..Default::default()
            }),
            defender: Some(sonettobuf::FightTeam {
                entitys: vec![entity(-1, 0)],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut tx = pool.begin().await.unwrap();
    let preview = settle_in_transaction(&mut tx, 18, &active).await.unwrap().unwrap();
    assert_eq!(preview.result, Some(1));
    assert_eq!(preview.theme.as_ref().unwrap().pass_max_layer_id, Some(episode.layer_id));
    assert_eq!(preview.theme.as_ref().unwrap().boss.as_ref().unwrap().level, Some(5));
    tx.rollback().await.unwrap();
    assert!(
        tower_compose::get_theme_state(&pool, 18, episode.theme_id)
            .await
            .unwrap()
            .is_none()
    );

    let mut tx = pool.begin().await.unwrap();
    settle_in_transaction(&mut tx, 18, &active).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        tower_compose::get_theme_state(&pool, 18, episode.theme_id)
            .await
            .unwrap()
            .unwrap()
            .pass_max_layer_id,
        episode.layer_id
    );
}

#[tokio::test]
async fn compose_unlock_requires_prior_layer_and_configured_high_score() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (19, 'compose-unlock', 0, 0)")
        .execute(&pool).await.unwrap();
    let params = |layer| serde_json::json!({"themeId":1,"layerId":layer,"planeId":0}).to_string();

    assert!(ensure_unlocked(&pool, 19, 90400101, Some(&params(1))).await.is_ok());
    assert!(ensure_unlocked(&pool, 19, 90400102, Some(&params(2))).await.is_err());
    assert!(ensure_unlocked(&pool, 19, 90400103, None).await.is_err());
    assert!(ensure_unlocked(&pool, 19, 10001, None).await.is_ok());
    sqlx::query("INSERT INTO user_tower_compose_theme_state (user_id, theme_id, pass_max_layer_id, high_score, updated_at) VALUES (19, 1, 3, 599, 0)")
        .execute(&pool).await.unwrap();
    assert!(ensure_unlocked(&pool, 19, 90400103, Some(&params(4))).await.is_err());
    sqlx::query("UPDATE user_tower_compose_theme_state SET high_score = 600 WHERE user_id = 19 AND theme_id = 1")
        .execute(&pool).await.unwrap();
    assert!(ensure_unlocked(&pool, 19, 90400103, Some(&params(4))).await.is_ok());
    sqlx::query("UPDATE user_tower_compose_theme_state SET pass_max_layer_id = 6, high_score = 1599 WHERE user_id = 19 AND theme_id = 1")
        .execute(&pool).await.unwrap();
    assert!(ensure_unlocked(&pool, 19, 90400105, Some(&params(7))).await.is_err());
    sqlx::query("UPDATE user_tower_compose_theme_state SET high_score = 1600 WHERE user_id = 19 AND theme_id = 1")
        .execute(&pool).await.unwrap();
    assert!(ensure_unlocked(&pool, 19, 90400105, Some(&params(7))).await.is_ok());
}
