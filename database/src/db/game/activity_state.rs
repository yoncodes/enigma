use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

pub type ActivityStates = HashMap<i32, (i32, i32, String)>;

pub struct ActivityStateSet<'a> {
    pub kind: ActivityStateKind,
    pub entry_id: i32,
    pub state: i32,
    pub progress: i32,
    pub ext: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityStateKind {
    Act101Day,
    Act101Once,
    Act125Episode,
    Act160Mission,
    Act165Story,
    Act208Bonus,
    Act209Layer,
    Act212Bonus,
    Act101SpBonus,
    Act186SpBonus,
    ActivityNewStage,
    ActivityPermanentUnlock,
    Act189OnceBonus,
    Act146Episode,
    Act104Episode,
    Act104Special,
    Act172UseItemTask,
    Act186Task,
    Act154Puzzle,
    Act216Task,
    Act216Flag,
    Act136Select,
    Act152Present,
    Act196Gain,
    Act197PoolGain,
    Act198Gain,
    Act206Chosen,
    Act205Game,
    Act221Summon,
    FairylandPuzzle,
    FairylandDialog,
    FairylandElement,
    CritterBook,
    Act104AfterStory,
    Act104Story,
    Act104PopSummary,
    Act128BossScore,
}

impl ActivityStateKind {
    pub const fn id(self) -> i32 {
        match self {
            Self::Act101Day => 1,
            Self::Act101Once => 2,
            Self::Act125Episode => 3,
            Self::Act160Mission => 4,
            Self::Act165Story => 5,
            Self::Act208Bonus => 6,
            Self::Act209Layer => 7,
            Self::Act212Bonus => 8,
            Self::Act101SpBonus => 9,
            Self::Act186SpBonus => 10,
            Self::ActivityNewStage => 11,
            Self::ActivityPermanentUnlock => 12,
            Self::Act189OnceBonus => 13,
            Self::Act146Episode => 14,
            Self::Act104Episode => 15,
            Self::Act104Special => 16,
            Self::Act172UseItemTask => 17,
            Self::Act186Task => 18,
            Self::Act154Puzzle => 19,
            Self::Act216Task => 20,
            Self::Act216Flag => 21,
            Self::Act136Select => 22,
            Self::Act152Present => 23,
            Self::Act196Gain => 24,
            Self::Act197PoolGain => 25,
            Self::Act198Gain => 26,
            Self::Act206Chosen => 27,
            Self::Act205Game => 28,
            Self::Act221Summon => 29,
            Self::FairylandPuzzle => 30,
            Self::FairylandDialog => 31,
            Self::FairylandElement => 32,
            Self::CritterBook => 33,
            Self::Act104AfterStory => 34,
            Self::Act104Story => 35,
            Self::Act104PopSummary => 36,
            Self::Act128BossScore => 37,
        }
    }
}

pub async fn get(
    db: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    kind: ActivityStateKind,
) -> Result<ActivityStates> {
    let rows = sqlx::query_as::<_, (i32, i32, i32, String)>(
        "SELECT entry_id, state, progress, ext
         FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(kind.id())
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(entry_id, state, progress, ext)| (entry_id, (state, progress, ext)))
        .collect())
}

pub async fn set(
    db: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    state: ActivityStateSet<'_>,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, kind, entry_id)
         DO UPDATE SET
            state = excluded.state,
            progress = excluded.progress,
            ext = excluded.ext,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(state.kind.id())
    .bind(state.entry_id)
    .bind(state.state)
    .bind(state.progress)
    .bind(state.ext)
    .bind(now)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn transition_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    activity_id: i32,
    expected_state: i32,
    state: ActivityStateSet<'_>,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let updated = sqlx::query(
        "UPDATE user_activity_state
         SET state = ?, progress = ?, ext = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ? AND state = ?",
    )
    .bind(state.state)
    .bind(state.progress)
    .bind(state.ext)
    .bind(now)
    .bind(user_id)
    .bind(activity_id)
    .bind(state.kind.id())
    .bind(state.entry_id)
    .bind(expected_state)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 1 {
        return Ok(true);
    }
    if expected_state != 0 {
        return Ok(false);
    }
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(state.kind.id())
    .bind(state.entry_id)
    .bind(state.state)
    .bind(state.progress)
    .bind(state.ext)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(inserted.rows_affected() == 1)
}

pub async fn get_activity_flags(
    db: &SqlitePool,
    user_id: i64,
    kind: ActivityStateKind,
) -> Result<HashSet<i32>> {
    let rows = sqlx::query_scalar(
        "SELECT activity_id
         FROM user_activity_state
         WHERE user_id = ? AND kind = ? AND state != 0",
    )
    .bind(user_id)
    .bind(kind.id())
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().collect())
}

pub async fn set_activity_flag(
    db: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    kind: ActivityStateKind,
    enabled: bool,
) -> Result<()> {
    set(
        db,
        user_id,
        activity_id,
        ActivityStateSet {
            kind,
            entry_id: 0,
            state: i32::from(enabled),
            progress: 0,
            ext: "",
        },
    )
    .await
}
