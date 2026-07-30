use database::db::game::equipment;
use sqlx::SqlitePool;

#[tokio::test]
async fn new_equipment_uses_configured_categories_for_default_lock() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let high_rarity_normal = tables
        .equip
        .iter()
        .find(|equip| equip.rare >= 4 && equip.is_exp_equip == 0 && equip.is_sp_refine == 0)
        .unwrap();
    let experience = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let special_refine = tables
        .equip
        .iter()
        .find(|equip| equip.is_sp_refine != 0)
        .unwrap();
    let low_rarity_normal = tables
        .equip
        .iter()
        .find(|equip| equip.rare < 4 && equip.is_exp_equip == 0 && equip.is_sp_refine == 0)
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'equipment-locks', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    for (config, expected_lock) in [
        (high_rarity_normal, true),
        (experience, false),
        (special_refine, false),
        (low_rarity_normal, false),
    ] {
        let uid = equipment::add_equipment(&pool, 1, config.id, 1)
            .await
            .unwrap()[0];
        let stored = equipment::get_equipment_by_uid(&pool, 1, uid)
            .await
            .unwrap();
        assert_eq!(stored.is_lock, expected_lock, "equipment {}", config.id);
    }
}
