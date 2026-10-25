use super::RoomManager;
use common::types::manufacture_slot_state::ManufactureSlotState;
use sonettobuf::MaterialData;
use sqlx::SqlitePool;

#[tokio::test]
async fn acceleration_rejects_empty_and_completed_slots_without_spending() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (22, 'manufacture', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (22, 123, 2)")
        .execute(&pool)
        .await
        .unwrap();
    for (slot_id, production_id, state) in [
        (1, 0, ManufactureSlotState::None),
        (2, 1, ManufactureSlotState::Complete),
    ] {
        sqlx::query(
            "INSERT INTO user_manufacture_slots
             (user_id, building_uid, slot_id, priority, production_id, slot_status,
              inventory_count, begin_time, next_finish_time, pause_time)
             VALUES (22, 1, ?, 0, ?, ?, 0, 0, 0, 0)",
        )
        .bind(slot_id)
        .bind(production_id)
        .bind(state.id())
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            RoomManager::new(22)
                .accelerate_manufacture(
                    &pool,
                    1,
                    slot_id,
                    Some(MaterialData {
                        materil_type: Some(1),
                        materil_id: Some(123),
                        quantity: Some(1),
                    }),
                    config::configs::get(),
                )
                .await
                .is_err()
        );
    }

    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 22 AND item_id = 123")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, 2);
}
