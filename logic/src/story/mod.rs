use crate::{error::AppError, reward};
use database::db::game::{necrologist_story, stories};
use sonettobuf::{
    GetHeroStoryReply, GetNecrologistStoryReply, GetStoryFinishReply, GetStoryReply,
    NecrologistStory, NecrologistStoryPlotInfo, NecrologistStorySituationValue,
    UnlocHeroStoryReply, UpdateNecrologistStoryReply, UpdateStoryReply,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

mod schedule;

pub struct StoryUpdate {
    pub reply: UpdateStoryReply,
    pub finished_story_id: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
pub struct StoryManager {
    player_id: i64,
}

impl StoryManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn get(&self, db: &SqlitePool) -> Result<GetStoryReply, AppError> {
        Ok(GetStoryReply {
            finish_list: stories::get_finished_stories(db, self.player_id).await?,
            processing_list: stories::get_processing_stories(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn finish_state(
        &self,
        db: &SqlitePool,
        story_id: i32,
    ) -> Result<GetStoryFinishReply, AppError> {
        Ok(GetStoryFinishReply {
            is_finish: Some(stories::is_story_finished(db, self.player_id, story_id).await?),
        })
    }

    pub async fn update(
        &self,
        db: &SqlitePool,
        story_id: i32,
        step_id: i32,
        favor: i32,
    ) -> Result<StoryUpdate, AppError> {
        if step_id == -1 {
            stories::finish_story(db, self.player_id, story_id).await?;
            return Ok(StoryUpdate {
                reply: UpdateStoryReply {},
                finished_story_id: Some(story_id),
            });
        }

        stories::update_processing_story(db, self.player_id, story_id, step_id, favor).await?;
        Ok(StoryUpdate {
            reply: UpdateStoryReply {},
            finished_story_id: None,
        })
    }

    pub async fn hero_story(&self, db: &SqlitePool) -> Result<GetHeroStoryReply, AppError> {
        stories::sync_hero_story_states(db, self.player_id, config::configs::get()).await?;
        let states = stories::get_hero_story_states(db, self.player_id).await?;

        Ok(GetHeroStoryReply {
            new_story_list: states
                .iter()
                .filter(|state| state.is_new)
                .map(|state| state.story_id)
                .collect(),
            story_infos: states.into_iter().map(Into::into).collect(),
            times: schedule::times(),
            left_num: Some(0),
            today_exchange: Some(0),
            week_progress: Some(0),
            week_has_get: Some(false),
        })
    }

    pub async fn unlock_hero_story(
        &self,
        db: &SqlitePool,
        story_id: i32,
    ) -> Result<(UnlocHeroStoryReply, reward::ConsumedRewards), AppError> {
        let story = config::configs::get()
            .hero_story
            .get(story_id)
            .ok_or(AppError::InvalidRequest)?;
        let cost = if schedule::is_active(story_id, common::time::ServerTime::now_sec_i32()) {
            &story.unlock
        } else {
            &story.permanent_unlock
        };
        let cost = reward::parse_strict(cost)?;

        stories::sync_hero_story_states(db, self.player_id, config::configs::get()).await?;
        let mut tx = db.begin().await?;
        let state = stories::unlock_hero_story(&mut tx, self.player_id, story_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let consumed = reward::RewardManager::new(self.player_id)
            .consume(&mut tx, &cost)
            .await?;
        tx.commit().await?;

        Ok((
            UnlocHeroStoryReply {
                info: Some(state.into()),
            },
            consumed,
        ))
    }

    pub async fn necrologist_story(
        &self,
        db: &SqlitePool,
        story_id: i32,
        tables: &config::GameDB,
    ) -> Result<GetNecrologistStoryReply, AppError> {
        necrologist_story::sync_stories(db, self.player_id, story_id, tables).await?;
        let states = necrologist_story::get_stories(db, self.player_id, story_id, tables).await?;
        let mut story = Vec::with_capacity(states.len());

        for state in states {
            let plot_infos = necrologist_story::get_plots(db, self.player_id, state.story_id)
                .await?
                .into_iter()
                .map(|plot| {
                    let values = serde_json::from_str::<BTreeMap<String, i32>>(&plot.values_json)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(key, value)| NecrologistStorySituationValue {
                            key: Some(key),
                            value: Some(value),
                        })
                        .collect();

                    NecrologistStoryPlotInfo {
                        id: Some(plot.plot_id),
                        state: Some(plot.state),
                        values,
                        selected_options: serde_json::from_str(&plot.selected_options_json)
                            .unwrap_or_default(),
                        unlock_end_ids: serde_json::from_str(&plot.unlock_end_ids_json)
                            .unwrap_or_default(),
                        last_selected_options: serde_json::from_str(
                            &plot.last_selected_options_json,
                        )
                        .unwrap_or_default(),
                        last_end_id: Some(plot.last_end_id),
                    }
                })
                .collect();

            story.push(NecrologistStory {
                story_id: Some(state.story_id),
                info: Some(state.info),
                plot_infos,
            });
        }

        Ok(GetNecrologistStoryReply { story })
    }

    pub async fn update_necrologist_story(
        &self,
        db: &SqlitePool,
        story_id: i32,
        info: String,
        plot_infos: Vec<NecrologistStoryPlotInfo>,
    ) -> Result<UpdateNecrologistStoryReply, AppError> {
        necrologist_story::update_story(
            db,
            self.player_id,
            story_id,
            info.clone(),
            plot_infos.clone(),
        )
        .await?;

        Ok(UpdateNecrologistStoryReply {
            story_id: Some(story_id),
            info: Some(info),
            plot_infos,
        })
    }
}

#[cfg(test)]
mod tests;
