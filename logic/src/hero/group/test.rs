use super::{
    HeroManager, hero_group_snapshots, model,
    snapshot::{overlay_snapshot_catalog, snapshot_group},
};
use crate::types::hero_group_snapshot_type::HeroGroupSnapshotType;
use database::models::game::hero_group_snapshots::HeroGroupSnapshotInfo;
use sonettobuf::{FightEquip, FightGroup, HeroGroupEquip};
use sqlx::SqlitePool;

const COMMON_SNAPSHOT_ID: i32 = HeroGroupSnapshotType::Common.id();

#[test]
fn hero_group_name_uses_configured_character_limit() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let heroes = HeroManager::new(1);
    assert!(heroes.check_group_name("队伍一").is_ok());
    assert!(heroes.check_group_name("").is_err());
    assert!(heroes.check_group_name("12345678901").is_err());
}

#[test]
fn all_snapshot_request_overlays_saved_data_on_the_protocol_catalog() {
    let snapshots = overlay_snapshot_catalog(vec![HeroGroupSnapshotInfo {
        snapshot_id: HeroGroupSnapshotType::TowerPermanentAndLimit.id(),
        hero_group_snapshots: Vec::new(),
        sort_sub_ids: vec![4, 2],
    }]);

    assert_eq!(snapshots.len(), HeroGroupSnapshotType::ALL_DESCENDING.len());
    assert_eq!(snapshots[0].snapshot_id, HeroGroupSnapshotType::Abyss.id());
    assert_eq!(snapshots.last().unwrap().snapshot_id, COMMON_SNAPSHOT_ID);
    assert_eq!(
        snapshots
            .iter()
            .find(|snapshot| {
                snapshot.snapshot_id == HeroGroupSnapshotType::TowerPermanentAndLimit.id()
            })
            .unwrap()
            .sort_sub_ids,
        [4, 2]
    );
}

#[test]
fn snapshot_preserves_slot_placeholders() {
    let group = snapshot_group(
        1,
        FightGroup {
            hero_list: vec![0, 12],
            equips: vec![FightEquip {
                hero_uid: Some(0),
                equip_uid: vec![0],
                ..Default::default()
            }],
            activity104_equips: vec![FightEquip {
                hero_uid: Some(0),
                equip_uid: vec![0, 0],
                ..Default::default()
            }],
            ..Default::default()
        },
    );

    assert_eq!(group.hero_list, [0, 12]);
    assert_eq!(group.equips[0].equip_uids, [0]);
    assert_eq!(group.activity104_equips[0].equip_uids, [0, 0]);
    let proto: sonettobuf::HeroGroupInfo = group.into();
    assert_eq!(proto.activity104_equips.len(), 5);
}

#[tokio::test]
async fn common_group_rename_keeps_snapshot_in_sync() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (12, 'group', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO hero_groups_common
             (user_id, group_id, name, cloth_id, assist_boss_id, created_at, updated_at)
             VALUES (12, 1, '', 1, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let snapshot_id = sqlx::query(
        "INSERT INTO hero_group_snapshots
             (user_id, snapshot_id, created_at, updated_at) VALUES (12, ?, 0, 0)",
    )
    .bind(COMMON_SNAPSHOT_ID)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO hero_group_snapshot_groups
             (snapshot_id, group_id, name, cloth_id, assist_boss_id)
             VALUES (?, 1, '', 1, 0)",
    )
    .bind(snapshot_id)
    .execute(&pool)
    .await
    .unwrap();

    HeroManager::new(12)
        .update_group_name(&pool, COMMON_SNAPSHOT_ID, 1, "Alpha".into())
        .await
        .unwrap();

    let common: String =
        sqlx::query_scalar("SELECT name FROM hero_groups_common WHERE user_id = 12")
            .fetch_one(&pool)
            .await
            .unwrap();
    let snapshot: String = sqlx::query_scalar("SELECT name FROM hero_group_snapshot_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((common.as_str(), snapshot.as_str()), ("Alpha", "Alpha"));
}

#[tokio::test]
async fn group_equipment_rejects_invalid_assignments_without_clearing_the_slot() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let universal_id = tables.equip_universal_refine_id().unwrap();
    let normal = tables
        .equip
        .iter()
        .find(|equip| tables.is_normal_equipment(equip))
        .unwrap();
    let experience = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let special_refine = tables
        .equip
        .iter()
        .find(|equip| equip.is_sp_refine != 0 && equip.id != universal_id)
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (19, 'equip', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let group_row = sqlx::query(
        "INSERT INTO hero_groups_common
             (user_id, group_id, name, cloth_id, assist_boss_id, created_at, updated_at)
         VALUES (19, 1, '', 1, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    for (position, hero_uid) in [(0, 10_i64), (1, 11), (2, 0)] {
        sqlx::query(
            "INSERT INTO hero_group_members (hero_group_id, hero_uid, position)
             VALUES (?, ?, ?)",
        )
        .bind(group_row)
        .bind(hero_uid)
        .bind(position)
        .execute(&pool)
        .await
        .unwrap();
    }
    for (uid, equip_id) in [
        (100_i64, normal.id),
        (101, experience.id),
        (102, special_refine.id),
    ] {
        sqlx::query(
            "INSERT INTO equipment
                 (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
                  created_at, updated_at)
             VALUES (?, 19, ?, 1, 0, 0, 1, 0, 1, 0, 0)",
        )
        .bind(uid)
        .bind(equip_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let heroes = HeroManager::new(19);
    assert!(
        heroes
            .update_group(
                &pool,
                sonettobuf::HeroGroupInfo {
                    group_id: 1,
                    hero_list: vec![10, 11, 0],
                    name: Some(String::new()),
                    cloth_id: Some(0),
                    equips: vec![HeroGroupEquip {
                        index: Some(0),
                        equip_uid: vec![101],
                    }],
                    ..Default::default()
                },
            )
            .await
            .is_err()
    );
    heroes
        .set_group_equip(
            &pool,
            1,
            HeroGroupEquip {
                index: Some(0),
                equip_uid: vec![100],
            },
        )
        .await
        .unwrap();
    assert!(
        heroes
            .set_group_equip(
                &pool,
                1,
                HeroGroupEquip {
                    index: Some(0),
                    equip_uid: vec![101],
                },
            )
            .await
            .is_err()
    );
    assert!(
        heroes
            .set_group_equip(
                &pool,
                1,
                HeroGroupEquip {
                    index: Some(0),
                    equip_uid: vec![102],
                },
            )
            .await
            .is_err()
    );
    assert!(
        heroes
            .set_group_equip(
                &pool,
                1,
                HeroGroupEquip {
                    index: Some(1),
                    equip_uid: vec![100],
                },
            )
            .await
            .is_err()
    );
    for equip in [
        HeroGroupEquip {
            index: Some(-1),
            equip_uid: vec![0],
        },
        HeroGroupEquip {
            index: Some(0),
            equip_uid: Vec::new(),
        },
        HeroGroupEquip {
            index: Some(0),
            equip_uid: vec![999],
        },
    ] {
        assert!(heroes.set_group_equip(&pool, 1, equip).await.is_err());
    }
    heroes
        .set_group_equip(
            &pool,
            1,
            HeroGroupEquip {
                index: Some(2),
                equip_uid: vec![0],
            },
        )
        .await
        .unwrap();

    let stored: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT index_slot, equip_uid FROM hero_group_equips
         WHERE hero_group_id = ? ORDER BY index_slot",
    )
    .bind(group_row)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(stored, [(0, 100), (2, 0)]);
}

#[tokio::test]
async fn common_preset_sort_and_delete_share_one_persisted_order() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (15, 'sort', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for group_id in [1, 2] {
        sqlx::query(
            "INSERT INTO hero_groups_common
                 (user_id, group_id, name, cloth_id, assist_boss_id, created_at, updated_at)
                 VALUES (15, ?, '', 1, 0, 0, 0)",
        )
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO hero_group_types
             (user_id, type_id, current_select, group_id, created_at, updated_at)
             VALUES (15, 1, 1, NULL, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let group = |group_id| model::HeroGroupInfo {
        group_id,
        hero_list: Vec::new(),
        name: String::new(),
        cloth_id: 1,
        equips: Vec::new(),
        activity104_equips: Vec::new(),
        assist_boss_id: 0,
        params: String::new(),
    };
    hero_group_snapshots::save_hero_group_snapshot(
        &pool,
        15,
        COMMON_SNAPSHOT_ID,
        vec![group(1), group(2)],
        vec![1, 2],
    )
    .await
    .unwrap();

    assert_eq!(
        HeroManager::new(15)
            .update_group_sort(&pool, COMMON_SNAPSHOT_ID, vec![2, 1])
            .await
            .unwrap()
            .sort_sub_ids,
        vec![2, 1]
    );
    assert!(
        HeroManager::new(15)
            .update_group_sort(&pool, COMMON_SNAPSHOT_ID, vec![2, 2])
            .await
            .is_err()
    );
    assert_eq!(
        HeroManager::new(15)
            .delete_group(&pool, COMMON_SNAPSHOT_ID, 1)
            .await
            .unwrap()
            .sort_sub_ids,
        vec![2]
    );
    let selected: i32 =
        sqlx::query_scalar("SELECT current_select FROM hero_group_types WHERE user_id = 15")
            .fetch_one(&pool)
            .await
            .unwrap();
    let remaining: Vec<i32> = sqlx::query_scalar(
        "SELECT group_id FROM hero_groups_common WHERE user_id = 15 ORDER BY group_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!((selected, remaining), (2, vec![2]));
}

#[tokio::test]
async fn update_group_preserves_the_owned_loadout_and_params() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (18, 'loadout', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_cloths (user_id, cloth_id) VALUES (18, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO hero_groups_common
             (user_id, group_id, name, cloth_id, assist_boss_id, created_at, updated_at)
             VALUES (18, 1, '', 1, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let heroes = HeroManager::new(18);
    let reply = heroes
        .update_group(
            &pool,
            sonettobuf::HeroGroupInfo {
                group_id: 1,
                hero_list: vec![0, 0, 0, 0],
                name: Some("Alpha".into()),
                cloth_id: Some(1),
                assist_boss_id: Some(7),
                params: Some("mode=normal".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        reply
            .group_info
            .as_ref()
            .and_then(|group| group.params.as_deref()),
        Some("mode=normal")
    );

    let common = database::db::game::hero_groups::get_hero_group(&pool, 18, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            common.hero_list.as_slice(),
            common.name.as_str(),
            common.params.as_str()
        ),
        (&[0, 0, 0, 0][..], "Alpha", "mode=normal")
    );
    hero_group_snapshots::save_hero_group_snapshot(
        &pool,
        18,
        COMMON_SNAPSHOT_ID,
        vec![common],
        vec![1],
    )
    .await
    .unwrap();
    let snapshot = hero_group_snapshots::get_hero_group_snapshot(&pool, 18, COMMON_SNAPSHOT_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.hero_group_snapshots[0].params, "mode=normal");

    assert!(
        heroes
            .update_group(
                &pool,
                sonettobuf::HeroGroupInfo {
                    group_id: 1,
                    cloth_id: Some(99),
                    ..Default::default()
                },
            )
            .await
            .is_err()
    );
}
