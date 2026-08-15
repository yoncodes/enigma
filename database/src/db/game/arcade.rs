use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone, Debug, FromRow)]
pub struct OutsideState {
    pub activity_id: i32,
    pub character_id: i32,
    pub x: i32,
    pub y: i32,
    pub dir: i32,
    pub score: i32,
    pub hotfix: String,
}

#[derive(Clone, Copy, Debug, FromRow)]
pub struct AttrState {
    pub attr_id: i32,
    pub base: i32,
    pub rate: i32,
    pub extra: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct NewOutsideState {
    pub activity_id: i32,
    pub character_id: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct TalentUpgradeCost {
    pub talent_id: i32,
    pub expected_level: i32,
    pub attr_id: i32,
    pub amount: i32,
    pub attr_min: i32,
}

pub async fn get_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Option<OutsideState>> {
    sqlx::query_as(
        "SELECT activity_id, character_id, x, y, dir, score, hotfix
         FROM user_arcade_outside WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_optional(pool)
    .await
}

pub async fn create_state(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    state: NewOutsideState,
    difficulty_id: i32,
    collection_id: i32,
    updated_at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_arcade_outside
         (user_id, activity_id, character_id, x, y, dir, score, hotfix, updated_at)
         VALUES (?, ?, ?, ?, ?, 0, 0, '[\"\"]', ?)
         ON CONFLICT(user_id, activity_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(state.activity_id)
    .bind(state.character_id)
    .bind(state.x)
    .bind(state.y)
    .bind(updated_at)
    .execute(&mut **tx)
    .await?;
    for (sql, value) in [
        (
            "INSERT INTO user_arcade_unlock_roles (user_id, activity_id, character_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
            state.character_id,
        ),
        (
            "INSERT INTO user_arcade_unlock_difficulties (user_id, activity_id, difficulty_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
            difficulty_id,
        ),
    ] {
        sqlx::query(sql)
            .bind(user_id)
            .bind(state.activity_id)
            .bind(value)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO user_arcade_books
         (user_id, activity_id, book_type, element_id, is_new)
         VALUES (?, ?, 1, ?, 1), (?, ?, 2, ?, 1)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(state.activity_id)
    .bind(state.character_id)
    .bind(user_id)
    .bind(state.activity_id)
    .bind(collection_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn move_player(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    x: i32,
    y: i32,
    updated_at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE user_arcade_outside SET x = ?, y = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(x)
    .bind(y)
    .bind(updated_at)
    .bind(user_id)
    .bind(activity_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn switch_character(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    character_id: i32,
    updated_at: i64,
) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_arcade_outside SET character_id = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND EXISTS (
             SELECT 1 FROM user_arcade_unlock_roles
             WHERE user_id = ? AND activity_id = ? AND character_id = ?)",
    )
    .bind(character_id)
    .bind(updated_at)
    .bind(user_id)
    .bind(activity_id)
    .bind(user_id)
    .bind(activity_id)
    .bind(character_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn get_talents(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<(i32, i32)>> {
    sqlx::query_as("SELECT talent_id, level FROM user_arcade_talents WHERE user_id = ? AND activity_id = ? ORDER BY talent_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn get_attrs(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<AttrState>> {
    sqlx::query_as("SELECT attr_id, base, rate, extra FROM user_arcade_attrs WHERE user_id = ? AND activity_id = ? ORDER BY attr_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn get_books(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<(i32, i32, bool)>> {
    sqlx::query_as("SELECT book_type, element_id, is_new FROM user_arcade_books WHERE user_id = ? AND activity_id = ? ORDER BY book_type, element_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn get_unlock_roles(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<i32>> {
    sqlx::query_scalar("SELECT character_id FROM user_arcade_unlock_roles WHERE user_id = ? AND activity_id = ? ORDER BY character_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn get_unlock_difficulties(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<i32>> {
    sqlx::query_scalar("SELECT difficulty_id FROM user_arcade_unlock_difficulties WHERE user_id = ? AND activity_id = ? ORDER BY difficulty_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn get_claims(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<i32>> {
    sqlx::query_scalar("SELECT reward_id FROM user_arcade_reward_claims WHERE user_id = ? AND activity_id = ? ORDER BY reward_id")
        .bind(user_id).bind(activity_id).fetch_all(pool).await
}

pub async fn upgrade_talent_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    cost: TalentUpgradeCost,
) -> sqlx::Result<Option<AttrState>> {
    let current_level = sqlx::query_scalar::<_, i32>(
        "SELECT level FROM user_arcade_talents
         WHERE user_id = ? AND activity_id = ? AND talent_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(cost.talent_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default();
    if current_level != cost.expected_level {
        return Ok(None);
    }

    let changed = sqlx::query(
        "UPDATE user_arcade_attrs SET base = base - ?
         WHERE user_id = ? AND activity_id = ? AND attr_id = ? AND base - ? >= ?",
    )
    .bind(cost.amount)
    .bind(user_id)
    .bind(activity_id)
    .bind(cost.attr_id)
    .bind(cost.amount)
    .bind(cost.attr_min)
    .execute(&mut **tx)
    .await?;
    if changed.rows_affected() != 1 {
        return Ok(None);
    }

    sqlx::query(
        "INSERT INTO user_arcade_talents (user_id, activity_id, talent_id, level)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, talent_id) DO UPDATE SET level = excluded.level",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(cost.talent_id)
    .bind(cost.expected_level + 1)
    .execute(&mut **tx)
    .await?;

    sqlx::query_as("SELECT attr_id, base, rate, extra FROM user_arcade_attrs WHERE user_id = ? AND activity_id = ? AND attr_id = ?")
        .bind(user_id).bind(activity_id).bind(cost.attr_id).fetch_optional(&mut **tx).await
}

pub async fn claim_reward_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    reward_id: i32,
    claimed_at: i64,
) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "INSERT INTO user_arcade_reward_claims (user_id, activity_id, reward_id, claimed_at)
         VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(reward_id)
    .bind(claimed_at)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}
