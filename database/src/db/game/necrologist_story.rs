use crate::models::game::necrologist_story::{NecrologistStoryPlotState, NecrologistStoryState};
use anyhow::Result;
use sonettobuf::NecrologistStoryPlotInfo;
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(crate) enum NecrologistStoryPlotStateValue {
    Lock = 1,
    Finish = 4,
}

impl NecrologistStoryPlotStateValue {
    pub(crate) const fn id(self) -> i32 {
        self as i32
    }
}

pub(crate) async fn finished_plots(
    conn: &mut SqliteConnection,
    user_id: i64,
) -> sqlx::Result<BTreeSet<(i32, i32)>> {
    Ok(sqlx::query_as::<_, (i32, i32)>(
        "SELECT plot.story_id, plot.plot_id
         FROM user_necrologist_story_plots plot
         JOIN user_hero_story_state story
           ON story.user_id = plot.user_id AND story.story_id = plot.story_id
         WHERE plot.user_id = ? AND plot.state = ? AND story.unlock = 1",
    )
    .bind(user_id)
    .bind(NecrologistStoryPlotStateValue::Finish.id())
    .fetch_all(conn)
    .await?
    .into_iter()
    .collect())
}

pub async fn get_stories(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
    tables: &config::GameDB,
) -> Result<Vec<NecrologistStoryState>> {
    let story_ids = story_ids(story_id, tables);

    if story_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", story_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT user_id, story_id, info, updated_at
         FROM user_necrologist_stories
         WHERE user_id = ? AND story_id IN ({})
         ORDER BY story_id",
        placeholders
    );
    let mut query = sqlx::query_as::<_, NecrologistStoryState>(&sql).bind(user_id);
    for story_id in story_ids {
        query = query.bind(story_id);
    }

    Ok(query.fetch_all(pool).await?)
}

pub async fn get_plots(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
) -> Result<Vec<NecrologistStoryPlotState>> {
    Ok(sqlx::query_as::<_, NecrologistStoryPlotState>(
        "SELECT user_id, story_id, plot_id, state, values_json,
                selected_options_json, unlock_end_ids_json,
                last_selected_options_json, last_end_id, updated_at
         FROM user_necrologist_story_plots
         WHERE user_id = ? AND story_id = ?
         ORDER BY plot_id",
    )
    .bind(user_id)
    .bind(story_id)
    .fetch_all(pool)
    .await?)
}

pub async fn update_story(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
    info: String,
    plot_infos: Vec<NecrologistStoryPlotInfo>,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        "INSERT INTO user_necrologist_stories (user_id, story_id, info, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, story_id) DO UPDATE SET
            info = excluded.info,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(story_id)
    .bind(info)
    .bind(now)
    .execute(pool)
    .await?;

    for plot in plot_infos {
        let Some(plot_id) = plot.id else {
            continue;
        };
        let values_json = serde_json::to_string(
            &plot
                .values
                .iter()
                .filter_map(|value| Some((value.key.clone()?, value.value?)))
                .collect::<BTreeMap<_, _>>(),
        )?;
        let selected_options_json = serde_json::to_string(&plot.selected_options)?;
        let unlock_end_ids_json = serde_json::to_string(&plot.unlock_end_ids)?;
        let last_selected_options_json = serde_json::to_string(&plot.last_selected_options)?;

        sqlx::query(
            "INSERT INTO user_necrologist_story_plots
                (user_id, story_id, plot_id, state, values_json,
                 selected_options_json, unlock_end_ids_json,
                 last_selected_options_json, last_end_id, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, story_id, plot_id) DO UPDATE SET
                state = excluded.state,
                values_json = excluded.values_json,
                selected_options_json = excluded.selected_options_json,
                unlock_end_ids_json = excluded.unlock_end_ids_json,
                last_selected_options_json = excluded.last_selected_options_json,
                last_end_id = excluded.last_end_id,
                updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(story_id)
        .bind(plot_id)
        .bind(
            plot.state
                .unwrap_or(NecrologistStoryPlotStateValue::Lock.id()),
        )
        .bind(values_json)
        .bind(selected_options_json)
        .bind(unlock_end_ids_json)
        .bind(last_selected_options_json)
        .bind(plot.last_end_id.unwrap_or_default())
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn sync_stories(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
    tables: &config::GameDB,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();
    let story_ids = story_ids(story_id, tables);

    for story_id in story_ids {
        sqlx::query(
            "INSERT INTO user_necrologist_stories (user_id, story_id, info, updated_at)
             VALUES (?, ?, '', ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(story_id)
        .bind(now)
        .execute(pool)
        .await?;

        // Plot state is client-authored save data. The client derives an untouched
        // story's initial lock/normal state from config and live omits plotInfos
        // until UpdateNecrologistStory persists progress. Remove rows produced by
        // the former eager reconciliation for stories that have never been saved.
        sqlx::query(
            "UPDATE user_necrologist_stories
             SET info = ''
             WHERE user_id = ? AND story_id = ? AND info = '{}'
               AND NOT EXISTS (
                   SELECT 1 FROM user_necrologist_story_plots plot
                   WHERE plot.user_id = ? AND plot.story_id = ?
                     AND (plot.values_json != '{}' OR plot.state NOT IN (1, 3))
               )",
        )
        .bind(user_id)
        .bind(story_id)
        .bind(user_id)
        .bind(story_id)
        .execute(pool)
        .await?;

        sqlx::query(
            "DELETE FROM user_necrologist_story_plots
             WHERE user_id = ? AND story_id = ?
               AND EXISTS (
                   SELECT 1 FROM user_necrologist_stories story
                   WHERE story.user_id = ? AND story.story_id = ? AND story.info = ''
               )",
        )
        .bind(user_id)
        .bind(story_id)
        .bind(user_id)
        .bind(story_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn story_ids(story_id: i32, tables: &config::GameDB) -> Vec<i32> {
    if story_id != 0 {
        return tables
            .hero_story_plot_group
            .iter()
            .any(|row| row.story_id == story_id)
            .then_some(vec![story_id])
            .unwrap_or_default();
    }

    tables
        .hero_story_plot_group
        .iter()
        .map(|row| row.story_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
