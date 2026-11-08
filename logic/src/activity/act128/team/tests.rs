use super::*;
use database::{
    db::game::{friends, player_infos},
    models::game::heros::UserHeroModel,
};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn validates_assists_identity_bonds_and_persisted_state() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let activity_id = 13919;
    let special = tables.character.get(3146).unwrap();
    let boost = tables.character.get(3134).unwrap();
    let boost_group = tables
        .activity128_bond_groups(activity_id)
        .unwrap()
        .into_iter()
        .find(|group| group.group_id == 10007)
        .unwrap();
    let career_group = tables
        .activity128_bond_groups(activity_id)
        .unwrap()
        .into_iter()
        .find(|group| group.group_id == 10001)
        .unwrap();
    let boss_id = tables
        .activity128_episode
        .iter()
        .find(|row| row.activity_id == activity_id)
        .unwrap()
        .stage;
    let other = tables
        .character
        .iter()
        .find(|hero| hero.id != special.id && hero.id != boost.id && hero.is_online == "1")
        .unwrap();
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (90, 'act128-owner', 0, 0), (91, 'act128-assist', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    player_infos::create_player_info(&db, 91, 0).await.unwrap();
    friends::add_friend(&db, 90, 91).await.unwrap();
    let owned_special = UserHeroModel::new(90, db.clone())
        .create_hero(special.id)
        .await
        .unwrap();
    let owned_boost = UserHeroModel::new(90, db.clone())
        .create_hero(boost.id)
        .await
        .unwrap();
    let assisted_special = UserHeroModel::new(91, db.clone())
        .create_hero(special.id)
        .await
        .unwrap();
    let assisted_other = UserHeroModel::new(91, db.clone())
        .create_hero(other.id)
        .await
        .unwrap();

    let mut owned = empty_slots();
    owned[0] = local_slot(owned_special);
    owned[1] = local_slot(owned_boost);
    let reply = set(
        &db,
        90,
        activity_id,
        boss_id,
        owned.clone(),
        empty_slots(),
        boost_group.group_id,
    )
    .await
    .unwrap();
    assert_eq!(
        reply.act_mode_team.unwrap().add_bond_group_id,
        Some(boost_group.group_id)
    );

    let mut assisted = empty_slots();
    assisted[0] = assist_slot(assisted_other, 91);
    let reply = set(&db, 90, activity_id, boss_id, assisted, empty_slots(), 0)
        .await
        .unwrap();
    let assist = reply.act_mode_team.unwrap().front_hero_infos[0]
        .assist_hero_info
        .clone()
        .unwrap();
    assert_eq!(assist.hero_uid, Some(assisted_other));
    assert_eq!(assist.is_friend, Some(true));

    let mut duplicate = owned.clone();
    duplicate[1] = assist_slot(assisted_special, 91);
    assert!(
        set(&db, 90, activity_id, boss_id, duplicate, empty_slots(), 0,)
            .await
            .is_err()
    );
    let mut self_assist = empty_slots();
    self_assist[0] = assist_slot(owned_special, 90);
    assert!(
        set(&db, 90, activity_id, boss_id, self_assist, empty_slots(), 0,)
            .await
            .is_err()
    );
    assert!(
        set(
            &db,
            90,
            activity_id,
            boss_id,
            owned.clone(),
            empty_slots(),
            career_group.group_id,
        )
        .await
        .is_err()
    );
    assert!(
        set(
            &db,
            90,
            activity_id,
            boss_id,
            vec![Act128ActModeHeroSlot::default(); 3],
            empty_slots(),
            0,
        )
        .await
        .is_err()
    );
    assert!(matches!(
        from_state(&db, 90, activity_id, boss_id, "{").await,
        Err(AppError::Serde(_))
    ));
    let invalid_stored = serde_json::to_string(&StoredTeam {
        front: vec![Act128ActModeHeroSlot::default(); 3],
        back: empty_slots(),
        add_bond_group_id: 0,
    })
    .unwrap();
    assert!(matches!(
        from_state(&db, 90, activity_id, boss_id, &invalid_stored).await,
        Err(AppError::InvalidRequest)
    ));
    let invalid_bond = serde_json::to_string(&StoredTeam {
        front: owned,
        back: empty_slots(),
        add_bond_group_id: career_group.group_id,
    })
    .unwrap();
    assert!(matches!(
        from_state(&db, 90, activity_id, boss_id, &invalid_bond).await,
        Err(AppError::InvalidRequest)
    ));
}

#[test]
fn added_bond_requires_the_exact_special_owner_and_highest_non_full_group() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let hero = |id, stone| tables.activity128_bond_hero(id, stone).unwrap();
    let activity_id = 13919;

    assert!(validate_add_bond(activity_id, 10007, &[hero(3146, 0), hero(3134, 0)]).is_ok());
    assert!(validate_add_bond(activity_id, 10007, &[hero(3051, 0), hero(3134, 0)]).is_err());
    assert!(
        validate_add_bond(
            activity_id,
            10008,
            &[hero(3146, 0), hero(3134, 0), hero(3139, 0), hero(3120, 0),],
        )
        .is_err()
    );
    assert!(
        validate_add_bond(
            activity_id,
            10011,
            &[
                hero(3146, 0),
                hero(3073, 0),
                hero(3076, 0),
                hero(3081, 0),
                hero(3094, 0),
            ],
        )
        .is_err()
    );
    let groups = tables.activity128_bond_groups(activity_id).unwrap();
    let damage = groups.iter().find(|group| group.group_id == 10019).unwrap();
    let support = groups.iter().find(|group| group.group_id == 10020).unwrap();
    assert!(damage.matches(&hero(3051, 0)));
    assert!(!support.matches(&hero(3051, 0)));
    assert!(damage.matches(&hero(3051, 305101)));
    assert!(support.matches(&hero(3051, 305101)));
}

fn empty_slots() -> Vec<Act128ActModeHeroSlot> {
    vec![Act128ActModeHeroSlot::default(); SLOTS_PER_LINE]
}

fn local_slot(uid: i64) -> Act128ActModeHeroSlot {
    Act128ActModeHeroSlot {
        uid: Some(uid),
        is_friend: Some(false),
        friend_user_id: Some(0),
    }
}

fn assist_slot(uid: i64, owner_id: i64) -> Act128ActModeHeroSlot {
    Act128ActModeHeroSlot {
        uid: Some(uid),
        is_friend: Some(true),
        friend_user_id: Some(owner_id),
    }
}
