use super::*;
use sqlx::sqlite::SqlitePoolOptions;

fn runtime(fight: sonettobuf::Fight) -> battle::engine::runtime::BattleRuntime {
    battle::engine::runtime::BattleRuntime::new(
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
        })
    );
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
    assert_eq!(
        settle_in_transaction(&mut tx, 18, &active)
            .await
            .unwrap()
            .unwrap()
            .result,
        Some(1)
    );
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
