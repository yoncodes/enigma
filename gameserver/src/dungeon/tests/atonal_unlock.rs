use super::*;
use database::db::game::battle::{create_fight_instance, NewFightInstance};

#[tokio::test]
async fn atonal_sequence_is_granted_atomically_on_13th_21_victory_and_only_once() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data.to_str().unwrap());
    let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    let user = 976003;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'atonal-victory', 0, 0)")
        .bind(user).execute(&db).await.unwrap();
    database::db::starter_data::load_all_starter_data(&db, user).await.unwrap();
    let ep = configs::get().episode.get(11321).unwrap();
    for attempt in 0..2 {
        let fight_id = create_fight_instance(&db, NewFightInstance { user_id: user, episode_id: ep.id,
            battle_id: ep.battle_id, multiplication: 1, entry_cost: "{}", checkpoint: "{}", created_at: 0 }).await.unwrap();
        let active = ActiveBattle { episode_id: ep.id, chapter_id: ep.chapter_id, battle_id: ep.battle_id,
            fight_id: Some(fight_id), ..Default::default() };
        let result = settle_active(&db, user, &active, DungeonCompletion { star: 2, total_round: 1,
            multiplier: 1, fight_group: None }, &Default::default()).await.unwrap();
        assert_eq!(result.rewards.cloth_updates.iter().filter(|cloth| cloth.cloth_id == Some(7)).count(), if attempt == 0 { 1 } else { 0 });
        let owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_cloths WHERE user_id = ? AND cloth_id = 7")
            .bind(user).fetch_one(&db).await.unwrap();
        assert_eq!(owned, 1);
    }
}
