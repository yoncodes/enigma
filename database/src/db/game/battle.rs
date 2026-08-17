use anyhow::{Result, ensure};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

pub struct NewFightInstance<'a> {
    pub user_id: i64,
    pub episode_id: i32,
    pub battle_id: i32,
    pub multiplication: i32,
    pub entry_cost: &'a str,
    pub checkpoint: &'a str,
    pub created_at: i64,
}

pub async fn create_fight_instance(
    pool: &SqlitePool,
    instance: NewFightInstance<'_>,
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let fight_id = create_fight_instance_in_transaction(&mut tx, instance).await?;
    tx.commit().await?;
    Ok(fight_id)
}

pub async fn create_fight_instance_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    instance: NewFightInstance<'_>,
) -> Result<i64> {
    let already_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM fight_instances WHERE user_id = ? AND active = 1
         )",
    )
    .bind(instance.user_id)
    .fetch_one(&mut **tx)
    .await?;
    ensure!(!already_active, "player already has an active fight");

    let result = sqlx::query(
        "INSERT INTO fight_instances
            (user_id, episode_id, battle_id, multiplication, entry_cost, checkpoint, active, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
    )
    .bind(instance.user_id)
    .bind(instance.episode_id)
    .bind(instance.battle_id)
    .bind(instance.multiplication.max(1))
    .bind(instance.entry_cost)
    .bind(instance.checkpoint)
    .bind(instance.created_at)
    .execute(&mut **tx)
    .await?;

    Ok(result.last_insert_rowid())
}

#[derive(Debug, FromRow)]
pub struct ActiveFightRecord {
    pub id: i64,
    pub episode_id: i32,
    pub battle_id: i32,
    pub multiplication: i32,
    pub entry_cost: String,
    pub checkpoint: String,
}

pub async fn load_active_fight(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Option<ActiveFightRecord>> {
    Ok(sqlx::query_as(
        "SELECT id, episode_id, battle_id, multiplication, entry_cost, checkpoint
         FROM fight_instances
         WHERE user_id = ? AND active = 1
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn finish_fight_instance(pool: &SqlitePool, user_id: i64, fight_id: i64) -> Result<()> {
    let mut tx = pool.begin().await?;
    finish_fight_instance_in_transaction(&mut tx, user_id, fight_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn finish_fight_instance_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    fight_id: i64,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE fight_instances
         SET active = 0, checkpoint = ''
         WHERE id = ? AND user_id = ? AND active = 1",
    )
    .bind(fight_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    ensure!(
        result.rows_affected() == 1,
        "active fight instance is missing"
    );
    Ok(())
}

pub async fn update_fight_checkpoint(
    pool: &SqlitePool,
    user_id: i64,
    fight_id: i64,
    checkpoint: &str,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE fight_instances
         SET checkpoint = ?
         WHERE id = ? AND user_id = ? AND active = 1",
    )
    .bind(checkpoint)
    .bind(fight_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    ensure!(
        result.rows_affected() == 1,
        "active fight instance is missing"
    );
    Ok(())
}

pub async fn save_round_operations(
    pool: &SqlitePool,
    user_id: i64,
    episode_id: i32,
    battle_id: i64,
    round_number: i32,
    cloth_skill_opers: Vec<sonettobuf::UseClothSkillOperRecord>,
    opers: Vec<sonettobuf::BeginRoundOper>,
) -> Result<()> {
    let cloth_json = serde_json::to_string(&cloth_skill_opers)?;
    let opers_json = serde_json::to_string(&opers)?;

    sqlx::query(
        "INSERT OR REPLACE INTO battle_replays
         (user_id, episode_id, battle_id, round_number, cloth_skill_opers, opers, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(episode_id)
    .bind(battle_id)
    .bind(round_number)
    .bind(cloth_json)
    .bind(opers_json)
    .bind(chrono::Utc::now().timestamp())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_battle_replay(
    pool: &SqlitePool,
    user_id: i64,
    episode_id: i32,
) -> Result<Vec<sonettobuf::FightRoundOperRecord>> {
    #[allow(dead_code)]
    #[derive(sqlx::FromRow)]
    struct ReplayRow {
        round_number: i32,
        cloth_skill_opers: String,
        opers: String,
    }

    let rows: Vec<ReplayRow> = sqlx::query_as(
        "SELECT round_number, cloth_skill_opers, opers
         FROM battle_replays
         WHERE user_id = ? AND episode_id = ?
         ORDER BY round_number",
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_all(pool)
    .await?;

    let mut records = Vec::new();
    for row in rows {
        let cloth_skill_opers: Vec<sonettobuf::UseClothSkillOperRecord> =
            serde_json::from_str(&row.cloth_skill_opers)?;
        let opers: Vec<sonettobuf::BeginRoundOper> = serde_json::from_str(&row.opers)?;

        records.push(sonettobuf::FightRoundOperRecord {
            cloth_skill_opers,
            opers,
        });
    }

    Ok(records)
}
