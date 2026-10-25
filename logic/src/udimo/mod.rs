use crate::error::AppError;
use database::models::game::heros::UserHeroModel;
use sonettobuf::{GetUdimoInfoReply, UdimoBackgroundNo, UdimoDecorationNo, UdimoNo};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct UdimoManager {
    player_id: i64,
}

impl UdimoManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetUdimoInfoReply, AppError> {
        let hero_times = UserHeroModel::new(self.player_id, db.clone())
            .get_hero_create_times()
            .await?
            .into_iter()
            .collect();

        Ok(build_info(tables, &hero_times))
    }
}

fn build_info(tables: &config::GameDB, hero_times: &HashMap<i32, i64>) -> GetUdimoInfoReply {
    GetUdimoInfoReply {
        udimos: tables
            .udimo
            .iter()
            .map(|entry| UdimoNo {
                udimo_id: Some(entry.id),
                is_use: Some(entry.default_use),
                get_time: Some(hero_times.get(&entry.hero_id).copied().unwrap_or_default()),
                fight_count: Some(0),
                hero_cover_day: Some(0),
                assist_count: Some(0),
                train_critter_count: Some(0),
            })
            .collect(),
        backgrounds: tables
            .udimo_background
            .iter()
            .map(|entry| UdimoBackgroundNo {
                background_id: Some(entry.id),
                is_use: Some(entry.default_use),
            })
            .collect(),
        decorations: tables
            .udimo_decoration
            .iter()
            .map(|entry| UdimoDecorationNo {
                decoration_id: Some(entry.id),
                is_use: Some(entry.default_use),
            })
            .collect(),
        weather: None,
    }
}

#[cfg(test)]
mod tests;
