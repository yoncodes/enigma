use super::activity_state::{self, ActivityStateKind, ActivityStateSet, ActivityStates};
use sqlx::{Sqlite, Transaction};

const GALLERY_ACTIVITY_ID: i32 = 0;

pub async fn boss_scores(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<Vec<(i32, i32, i32)>> {
    sqlx::query_as(
        "SELECT activity_id, entry_id, progress FROM user_activity_state
         WHERE user_id = ? AND kind = ?",
    )
    .bind(user_id)
    .bind(ActivityStateKind::Act128BossScore.id())
    .fetch_all(&mut **tx)
    .await
}

pub async fn gallery_claims(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<ActivityStates> {
    let rows = sqlx::query_as::<_, (i32, i32, i32, String)>(
        "SELECT entry_id, state, progress, ext FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ?",
    )
    .bind(user_id)
    .bind(GALLERY_ACTIVITY_ID)
    .bind(ActivityStateKind::Act128Gallery.id())
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(boss_type, accepted, highest, played)| (boss_type, (accepted, highest, played)))
        .collect())
}

pub async fn lock_gallery(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    boss_type: i32,
) -> sqlx::Result<()> {
    // Take the write lock before reading the cursor so concurrent claims cannot both pay.
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity_state
         (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, 0, 0, '', ?)",
    )
    .bind(user_id)
    .bind(GALLERY_ACTIVITY_ID)
    .bind(ActivityStateKind::Act128Gallery.id())
    .bind(boss_type)
    .bind(common::time::ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn save_gallery(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    boss_type: i32,
    previous: i32,
    accepted: i32,
    highest: i32,
) -> anyhow::Result<bool> {
    activity_state::transition_in_transaction(
        tx,
        user_id,
        GALLERY_ACTIVITY_ID,
        previous,
        ActivityStateSet {
            kind: ActivityStateKind::Act128Gallery,
            entry_id: boss_type,
            state: accepted,
            progress: highest,
            ext: "1",
        },
    )
    .await
}
