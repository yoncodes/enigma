use std::path::{Path, PathBuf};

use database::models::game::tower::TowerType;
use sonettobuf::{StartDungeonRequest, StartTowerBattleRequest};

use super::validate_battle_start;

fn tables() -> &'static config::GameDB {
    let data = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap()).unwrap();
    config::configs::get()
}

fn request(
    tower_type: TowerType,
    tower_id: i32,
    layer_id: i32,
    difficulty: i32,
    episode_id: i32,
) -> StartTowerBattleRequest {
    StartTowerBattleRequest {
        start_dungeon_request: Some(StartDungeonRequest {
            episode_id: Some(episode_id),
            ..Default::default()
        }),
        r#type: Some(tower_type.id()),
        tower_id: Some(tower_id),
        layer_id: Some(layer_id),
        difficulty: Some(difficulty),
        talent_plan_id: Some(0),
    }
}

#[test]
fn tower_episode_identity_is_derived_from_each_config_family() {
    let tables = tables();

    let permanent = tables.tower_permanent_episode.iter().next().unwrap();
    let permanent_episode = permanent
        .episode_ids
        .split('|')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Normal,
                0,
                permanent.layer_id,
                0,
                permanent_episode,
            ),
        )
        .is_ok()
    );

    let boss = tables.tower_boss_episode.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Boss,
                boss.tower_id,
                boss.layer_id,
                0,
                boss.episode_id,
            ),
        )
        .is_ok()
    );

    let teach = tables.tower_boss_teach.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Boss,
                teach.tower_id,
                0,
                teach.teach_id,
                teach.episode_id,
            ),
        )
        .is_ok()
    );

    let limited = tables.tower_limited_episode.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Limited,
                limited.season,
                limited.layer_id,
                limited.difficulty,
                limited.episode_id,
            ),
        )
        .is_ok()
    );
}

#[test]
fn related_but_mismatched_tower_coordinates_are_rejected() {
    let tables = tables();
    let boss = tables.tower_boss_episode.iter().next().unwrap();
    let mismatched = request(
        TowerType::Boss,
        boss.tower_id,
        boss.layer_id + 1,
        0,
        boss.episode_id,
    );

    assert!(validate_battle_start(tables, &mismatched).is_err());
}
