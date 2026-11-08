use crate::{
    db::game::explore::state::{insert, last_map, load, select, update},
    models::game::explore::state::{InteractState, MapState},
};
use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn snapshots_preserve_progress_and_selection_preserves_bag() -> Result<()> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    crate::run_migrations(&pool).await?;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'explore-storage', 0, 0)",
    )
    .execute(&pool)
    .await?;
    let mut conn = pool.acquire().await?;
    let progressed = MapState {
        map_id: 101,
        posx: 8,
        posy: -3,
        dir: 2,
        challenge_count: 4,
        interacts: vec![InteractState {
            id: 12,
            kind: 7,
            status: 3,
            status2: "saved".into(),
            posx: 5,
            posy: 6,
            dir: 1,
        }],
        area_ids: vec![2, 9],
        move_nodes: "8,-3|8,-2".into(),
        items: vec![],
        use_item_uid: 0,
    };
    assert_eq!(load(&mut conn, 1, progressed.map_id).await?, None);
    assert!(insert(&mut conn, 1, &progressed).await?);
    assert_eq!(
        load(&mut conn, 1, progressed.map_id).await?,
        Some(progressed.clone())
    );
    let mut moved = progressed.clone();
    moved.posx = 9;
    assert!(update(&mut conn, 1, &moved).await?);
    assert_eq!(load(&mut conn, 1, moved.map_id).await?, Some(moved.clone()));
    assert!(!update(&mut conn, 2, &progressed).await?);

    let initial = MapState {
        map_id: progressed.map_id,
        posx: 0,
        posy: 0,
        dir: 0,
        challenge_count: 0,
        interacts: vec![],
        area_ids: vec![],
        move_nodes: String::new(),
        items: vec![],
        use_item_uid: 0,
    };
    assert!(!insert(&mut conn, 1, &initial).await?);
    assert_eq!(load(&mut conn, 1, progressed.map_id).await?, Some(moved));

    assert_eq!(last_map(&mut conn, 1).await?, None);
    select(&mut conn, 1, progressed.map_id).await?;
    assert_eq!(last_map(&mut conn, 1).await?, Some(progressed.map_id));
    sqlx::query("UPDATE user_explore_info SET is_show_bag = 1 WHERE user_id = 1")
        .execute(&mut *conn)
        .await?;
    select(&mut conn, 1, 102).await?;
    assert_eq!(last_map(&mut conn, 1).await?, Some(102));
    let info: (i32, bool) =
        sqlx::query_as("SELECT last_map_id, is_show_bag FROM user_explore_info WHERE user_id = 1")
            .fetch_one(&mut *conn)
            .await?;
    assert_eq!(info, (102, true));

    sqlx::query("UPDATE user_explore_map_state SET snapshot = '{}' WHERE user_id = 1")
        .execute(&mut *conn)
        .await?;
    assert!(load(&mut conn, 1, progressed.map_id).await.is_err());
    Ok(())
}

#[test]
fn legacy_snapshot_defaults_item_state() -> Result<()> {
    let state: MapState = serde_json::from_str(
        r#"{"map_id":101,"posx":1,"posy":6,"dir":180,"challenge_count":0,"interacts":[],"area_ids":[1],"move_nodes":"1#6"}"#,
    )?;
    assert!(state.items.is_empty());
    assert_eq!(state.use_item_uid, 0);
    Ok(())
}
