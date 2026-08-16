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

#[derive(Clone, Debug, FromRow)]
pub struct InsideSave {
    pub difficulty: i32,
    pub snapshot: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct SettleBook {
    pub book_type: i32,
    pub element_id: i32,
    pub score: i32,
}

#[derive(Clone, Debug)]
pub struct SettleProjection {
    pub difficulty: i32,
    pub diamond_gain: i32,
    pub diamond_min: i32,
    pub diamond_max: i32,
    pub cassette_score: i32,
    pub books: Vec<SettleBook>,
    pub roles: Vec<i32>,
    pub unlock_difficulty: Option<i32>,
    pub is_win: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug)]
pub struct SettleResult {
    pub attr: AttrState,
    pub book_score: i32,
    pub new_roles: Vec<i32>,
    pub hotfix: String,
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

pub async fn get_inside_save(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Option<InsideSave>> {
    sqlx::query_as(
        "SELECT difficulty, snapshot FROM user_arcade_inside_saves
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_inside_save(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    difficulty: i32,
    snapshot: &[u8],
    updated_at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_arcade_inside_saves
         (user_id, activity_id, difficulty, snapshot, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id) DO UPDATE SET
             difficulty = excluded.difficulty,
             snapshot = excluded.snapshot,
             updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(difficulty)
    .bind(snapshot)
    .bind(updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn settle_inside_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    expected_snapshot: &[u8],
    projection: SettleProjection,
) -> sqlx::Result<Option<SettleResult>> {
    let deleted = sqlx::query(
        "DELETE FROM user_arcade_inside_saves
         WHERE user_id = ? AND activity_id = ? AND difficulty = ? AND snapshot = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(projection.difficulty)
    .bind(expected_snapshot)
    .execute(&mut **tx)
    .await?;
    if deleted.rows_affected() != 1 {
        return Ok(None);
    }

    let changed_attr = sqlx::query(
        "INSERT INTO user_arcade_attrs (user_id, activity_id, attr_id, base, rate, extra)
         VALUES (?, ?, 202, ?, 0, 0)
         ON CONFLICT(user_id, activity_id, attr_id) DO UPDATE SET
             base = base + excluded.base
         WHERE base + excluded.base BETWEEN ? AND ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(projection.diamond_gain)
    .bind(projection.diamond_min)
    .bind(projection.diamond_max)
    .execute(&mut **tx)
    .await?;
    if changed_attr.rows_affected() != 1 {
        return Ok(None);
    }

    let attr = sqlx::query_as::<_, AttrState>(
        "SELECT attr_id, base, rate, extra FROM user_arcade_attrs
         WHERE user_id = ? AND activity_id = ? AND attr_id = 202",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_one(&mut **tx)
    .await?;
    if attr.base < projection.diamond_min || attr.base > projection.diamond_max {
        return Ok(None);
    }

    let mut book_score = 0_i32;
    for book in projection.books {
        let inserted = sqlx::query(
            "INSERT INTO user_arcade_books
             (user_id, activity_id, book_type, element_id, is_new)
             VALUES (?, ?, ?, ?, 1) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(book.book_type)
        .bind(book.element_id)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected() == 1 {
            book_score = book_score
                .checked_add(book.score)
                .ok_or(sqlx::Error::Protocol("arcade book score overflow".into()))?;
        }
    }

    let mut new_roles = Vec::new();
    for role in projection.roles {
        let inserted = sqlx::query(
            "INSERT INTO user_arcade_unlock_roles (user_id, activity_id, character_id)
             VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(role)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected() == 1 {
            new_roles.push(role);
        }
    }

    if let Some(difficulty) = projection.unlock_difficulty {
        sqlx::query(
            "INSERT INTO user_arcade_unlock_difficulties (user_id, activity_id, difficulty_id)
             VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(difficulty)
        .execute(&mut **tx)
        .await?;
    }
    if projection.is_win {
        sqlx::query(
            "INSERT INTO user_arcade_completions
             (user_id, activity_id, difficulty, finish_count) VALUES (?, ?, ?, 1)
             ON CONFLICT(user_id, activity_id, difficulty) DO UPDATE SET
                 finish_count = finish_count + 1",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(projection.difficulty)
        .execute(&mut **tx)
        .await?;
    }

    let completions = sqlx::query_as::<_, (i32, i32)>(
        "SELECT difficulty, finish_count FROM user_arcade_completions
         WHERE user_id = ? AND activity_id = ? ORDER BY difficulty",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_all(&mut **tx)
    .await?;
    let hotfix = completions
        .iter()
        .map(|(difficulty, count)| format!("{difficulty}#{count}"))
        .collect::<Vec<_>>()
        .join("|");
    let stored_hotfix = serde_json::to_string(&vec![hotfix.clone()])
        .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
    let score_gain = projection
        .cassette_score
        .checked_add(book_score)
        .ok_or(sqlx::Error::Protocol("arcade score overflow".into()))?;
    let updated = sqlx::query(
        "UPDATE user_arcade_outside SET
             score = score + ?, hotfix = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ?
           AND score + ? BETWEEN 0 AND 2147483647",
    )
    .bind(score_gain)
    .bind(stored_hotfix)
    .bind(projection.updated_at)
    .bind(user_id)
    .bind(activity_id)
    .bind(score_gain)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Ok(None);
    }

    Ok(Some(SettleResult {
        attr,
        book_score,
        new_roles,
        hotfix,
    }))
}
