use std::path::{Path, PathBuf};

use database::{db::game::equipment, models::game::heros::UserHeroModel};

use battle::dungeon::{FightOptions, plan_roster};

use super::{build_fight, load_roster};

fn init_config() {
    let data = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap()).unwrap();
}

#[tokio::test]
async fn roster_adapter_loads_selected_and_linked_psychubes_once() {
    init_config();
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'battle-roster', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(1, db.clone());
    let hero_uid = heroes.create_hero(3149).await.unwrap();
    let primary_uid = equipment::add_equipment(&db, 1, 1571, 1).await.unwrap()[0];
    let linked_uid = equipment::add_equipment(&db, 1, 1572, 1).await.unwrap()[0];
    sqlx::query("UPDATE equipment SET refine_lv = 5 WHERE uid = ?")
        .bind(linked_uid)
        .execute(&db)
        .await
        .unwrap();
    let group = sonettobuf::FightGroup {
        hero_list: vec![hero_uid],
        equips: vec![sonettobuf::FightEquip {
            hero_uid: Some(hero_uid),
            equip_uid: vec![primary_uid],
            ..Default::default()
        }],
        ..Default::default()
    };

    let plan = plan_roster(10101, 10101, false, &group, None).unwrap();
    let roster = load_roster(&db, 1, &plan, &group).await.unwrap();
    assert_eq!(roster.fighters[0].hero.uid, hero_uid);
    assert_eq!(
        roster.fighters[0]
            .equips
            .iter()
            .map(|equip| (equip.uid, equip.equip_id, equip.refine_level))
            .collect::<Vec<_>>(),
        vec![(primary_uid, 1571, 1), (linked_uid, 1572, 5)]
    );

    let invalid = sonettobuf::FightGroup {
        equips: vec![sonettobuf::FightEquip {
            hero_uid: Some(hero_uid),
            equip_uid: vec![primary_uid, linked_uid],
            ..Default::default()
        }],
        ..group
    };
    let plan = plan_roster(10101, 10101, false, &invalid, None).unwrap();
    assert!(load_roster(&db, 1, &plan, &invalid).await.is_err());
}

#[tokio::test]
async fn invalid_requests_fail_before_roster_hydration() {
    init_config();
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let oversized = sonettobuf::FightGroup {
        hero_list: vec![90_000_001, 90_000_002, 90_000_003, 90_000_004],
        ..Default::default()
    };
    let error = build_fight(
        &db,
        1,
        10101,
        10101,
        &oversized,
        FightOptions::default(),
        None,
    )
    .await
    .err()
    .expect("oversized roster should fail");
    assert!(error.to_string().contains("invalid composition"));

    let compose = sonettobuf::FightGroup {
        params: Some(r#"{"buffParamsStr":"306600#0#0"}"#.to_owned()),
        ..Default::default()
    };
    let error = build_fight(
        &db,
        1,
        90400101,
        9001101,
        &compose,
        FightOptions::default(),
        Some(
            r#"{"supportAssistUid":90000001,"layerId":1,"supportId":306600,"supportAssistType":6,"themeId":1,"planeId":0}"#,
        ),
    )
    .await
    .err()
    .expect("forged Compose assist should fail");
    assert!(
        error
            .to_string()
            .contains("missing selected Tower Compose assist")
    );
}
