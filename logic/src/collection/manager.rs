use crate::{error::AppError, task::TaskEvent};
use database::{
    db::game::{achievements, antiques, dialogs, player_infos},
    models::game::achievements::Achievement,
};
use sonettobuf::{
    GetAchievementInfoReply, GetAntiqueInfoReply, GetDialogInfoReply, ReadNewAchievementReply,
    RecordDialogInfoReplay, ShowAchievementReply, UpdateAchievementPush,
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct CollectionManager {
    pub(super) player_id: i64,
    achievements: HashMap<i32, Achievement>,
    dialogs: HashSet<i32>,
}

impl CollectionManager {
    pub fn new(player_id: i64) -> Self {
        Self {
            player_id,
            achievements: HashMap::new(),
            dialogs: HashSet::new(),
        }
    }

    pub async fn sync_task_event(
        &mut self,
        db: &SqlitePool,
        event: TaskEvent,
    ) -> Result<Vec<Achievement>, AppError> {
        let achievements = achievements::sync_event(db, self.player_id, event).await?;
        self.track(&achievements);
        Ok(achievements)
    }

    pub fn record_updates(
        &mut self,
        achievements: Vec<Achievement>,
    ) -> Option<UpdateAchievementPush> {
        if achievements.is_empty() {
            return None;
        }
        self.track(&achievements);
        Some(UpdateAchievementPush {
            infos: achievements.into_iter().map(Into::into).collect(),
        })
    }

    fn track(&mut self, achievements: &[Achievement]) {
        self.achievements.extend(
            achievements
                .iter()
                .map(|achievement| (achievement.achievement_id, achievement.clone())),
        );
    }

    pub async fn achievement_info(
        &mut self,
        db: &SqlitePool,
    ) -> Result<GetAchievementInfoReply, AppError> {
        achievements::reconcile_snapshot(db, self.player_id).await?;
        let achievements = achievements::get_achievements(db, self.player_id).await?;
        self.achievements = achievements
            .iter()
            .map(|achievement| (achievement.achievement_id, achievement.clone()))
            .collect();

        Ok(GetAchievementInfoReply {
            infos: achievements.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn complete_all_achievements(
        &mut self,
        db: &SqlitePool,
    ) -> Result<UpdateAchievementPush, AppError> {
        let achievements = achievements::complete_all(db, self.player_id).await?;
        self.track(&achievements);
        Ok(UpdateAchievementPush {
            infos: achievements.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn read_new_achievement(
        &mut self,
        db: &SqlitePool,
        ids: Vec<i32>,
    ) -> Result<(ReadNewAchievementReply, UpdateAchievementPush), AppError> {
        let updated = achievements::clear_new_flags(db, self.player_id, ids.clone()).await?;
        for id in &ids {
            if let Some(achievement) = self.achievements.get_mut(id) {
                achievement.is_new = false;
            }
        }

        Ok((
            ReadNewAchievementReply { ids },
            UpdateAchievementPush {
                infos: updated.into_iter().map(Into::into).collect(),
            },
        ))
    }

    pub async fn show_achievement(
        &self,
        db: &SqlitePool,
        ids: Vec<i32>,
        group_id: Option<i32>,
    ) -> Result<ShowAchievementReply, AppError> {
        let show_achievement = encode_show_achievement(&ids, group_id);
        player_infos::set_show_achievement(db, self.player_id, show_achievement).await?;

        Ok(ShowAchievementReply { ids, group_id })
    }

    pub async fn dialog_info(&mut self, db: &SqlitePool) -> Result<GetDialogInfoReply, AppError> {
        let dialog_ids = dialogs::get_dialog_ids(db, self.player_id).await?;
        self.dialogs = dialog_ids.iter().copied().collect();

        Ok(GetDialogInfoReply { dialog_ids })
    }

    pub async fn record_dialog(
        &mut self,
        db: &SqlitePool,
        dialog_id: Option<i32>,
    ) -> Result<RecordDialogInfoReplay, AppError> {
        let dialog_id = dialog_id.ok_or(AppError::InvalidRequest)?;
        dialogs::add_dialog(db, self.player_id, dialog_id).await?;
        self.dialogs.insert(dialog_id);

        Ok(RecordDialogInfoReplay {
            dialog_id: Some(dialog_id),
        })
    }

    pub async fn antique_info(&self, db: &SqlitePool) -> Result<GetAntiqueInfoReply, AppError> {
        Ok(GetAntiqueInfoReply {
            antiques: antiques::get_user_antiques(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}

fn encode_show_achievement(ids: &[i32], group_id: Option<i32>) -> String {
    if ids.is_empty() {
        return String::new();
    }

    let tag = if group_id.unwrap_or_default() == 0 {
        "1"
    } else {
        "2"
    };
    let ids = ids.iter().map(i32::to_string).collect::<Vec<_>>().join("#");

    format!("{tag}:{ids}")
}

#[cfg(test)]
mod tests {
    use super::encode_show_achievement;

    #[test]
    fn encodes_single_and_group_display_strings() {
        assert_eq!(encode_show_achievement(&[1, 2], Some(0)), "1:1#2");
        assert_eq!(encode_show_achievement(&[3, 4], Some(12)), "2:3#4");
        assert_eq!(encode_show_achievement(&[], Some(12)), "");
    }
}
