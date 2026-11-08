use super::super::max_equip;
use sqlx::SqlitePool;

#[tokio::test]
async fn psychube_editor_uses_configured_maximum_progression() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let config = tables
        .equip
        .iter()
        .find(|equip| tables.is_normal_equipment(equip))
        .unwrap();
    let maximum = tables.max_equip_progression(config.rare).unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (43, 'psychube-editor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let uid = database::db::game::equipment::add_equipment(&pool, 43, config.id, 1)
        .await
        .unwrap()[0];

    let updated = max_equip(&pool, 43, uid).await.unwrap();

    assert_eq!(updated.level, Some(maximum.level));
    assert_eq!(updated.break_lv, Some(maximum.break_level));
}
