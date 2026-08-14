use crate::{
    GameDB, activity104_episode::Activity104Episode, activity104_retail::Activity104Retail,
    activity104_special::Activity104Special, activity104_trial::Activity104Trial,
    activity128_level::Activity128Level, activity165_step::Activity165Step,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity128Battle {
    pub activity_id: i32,
    pub boss_id: i32,
    pub target_model_ids: Vec<i32>,
}

impl GameDB {
    pub fn latest_open_activity_id(&self, type_id: i32) -> Option<i32> {
        self.activity
            .iter()
            .filter(|activity| {
                activity.type_id == type_id
                    && (activity.open_id == 0
                        || self
                            .open
                            .get(activity.open_id)
                            .is_some_and(|open| open.is_online != 0))
            })
            .map(|activity| activity.id)
            .max()
    }

    pub fn latest_activity104_id(&self) -> Option<i32> {
        self.activity104_episode
            .iter()
            .map(|row| row.activity_id)
            .max()
    }

    pub fn activity104_episodes(
        &self,
        activity_id: i32,
    ) -> impl Iterator<Item = &Activity104Episode> {
        self.activity104_episode
            .iter()
            .filter(move |row| row.activity_id == activity_id)
    }

    pub fn activity104_episode(&self, activity_id: i32, layer: i32) -> Option<&Activity104Episode> {
        self.activity104_episodes(activity_id)
            .find(|row| row.layer == layer)
    }

    pub fn activity104_specials(
        &self,
        activity_id: i32,
    ) -> impl Iterator<Item = &Activity104Special> {
        self.activity104_special
            .iter()
            .filter(move |row| row.activity_id == activity_id)
    }

    pub fn activity104_retails(
        &self,
        activity_id: i32,
    ) -> impl Iterator<Item = &Activity104Retail> {
        self.activity104_retail
            .iter()
            .filter(move |row| row.activity_id == activity_id)
    }

    pub fn activity104_trial(&self, activity_id: i32) -> Option<&Activity104Trial> {
        self.activity104_trial
            .iter()
            .find(|row| row.activity_id == activity_id)
    }

    pub fn activity128_battle(&self, episode_id: i32, battle_id: i32) -> Option<Activity128Battle> {
        if self.episode.get(episode_id)?.battle_id != battle_id {
            return None;
        }
        let episode = self
            .activity128_episode
            .iter()
            .find(|row| row.episode_id == episode_id)?;
        let boss = self
            .activity128_countboss
            .iter()
            .find(|row| row.battle_id == battle_id)?;

        Some(Activity128Battle {
            activity_id: episode.activity_id,
            boss_id: episode.stage,
            target_model_ids: boss
                .monster_id
                .split('#')
                .map(str::parse)
                .collect::<Result<_, _>>()
                .ok()?,
        })
    }

    pub fn activity128_rank_currency_id(&self) -> Option<i32> {
        let row = self.activity128_const.get(10)?;
        let mut fields = row.value2.split('#');
        match (fields.next(), fields.next(), fields.next()) {
            (Some("2"), Some(currency_id), None) => currency_id
                .parse()
                .ok()
                .filter(|currency_id| *currency_id > 0),
            _ => None,
        }
    }

    pub fn activity128_rank(&self, exp: i32) -> Option<i32> {
        if exp < 0 {
            return None;
        }

        let mut levels = self.activity128_level.iter().collect::<Vec<_>>();
        if levels.is_empty() {
            return None;
        }
        levels.sort_unstable_by_key(|row| row.player_level);
        let mut threshold: i32 = 0;
        let mut rank = 0;

        for (index, row) in levels.into_iter().enumerate() {
            if row.player_level != index as i32 + 1 || row.need_exp <= 0 {
                return None;
            }
            threshold = threshold.checked_add(row.need_exp)?;
            if exp >= threshold {
                rank = row.player_level;
            }
        }

        Some(rank)
    }

    pub fn activity128_milestone_levels(
        &self,
        claimed_level: i32,
        target_level: i32,
    ) -> Option<Vec<&Activity128Level>> {
        if claimed_level < 0 || target_level <= claimed_level {
            return None;
        }

        (claimed_level + 1..=target_level)
            .map(|level| {
                self.activity128_level
                    .iter()
                    .find(|row| row.player_level == level)
            })
            .collect()
    }

    pub fn activity165_step(&self, story_id: i32, step_id: i32) -> Option<&Activity165Step> {
        self.activity165_step
            .iter()
            .find(|row| row.belong_story_id == story_id && row.step_id == step_id)
    }

    pub fn activity165_steps(&self, story_id: i32) -> impl Iterator<Item = &Activity165Step> {
        self.activity165_step
            .iter()
            .filter(move |row| row.belong_story_id == story_id)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn act236_reward_rows_load_for_the_configured_activity() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = crate::init(&data_dir);
        let tables = crate::configs::get();
        let activity_id = tables.latest_open_activity_id(236).unwrap();
        let rows = tables
            .activity236
            .iter()
            .filter(|row| row.activity_id == activity_id)
            .collect::<Vec<_>>();

        assert_eq!(rows.len(), 9);
        assert_eq!((rows[0].cost, rows[0].reward.as_str()), (0, "2#2#100"));
        assert!(rows.windows(2).all(|rows| rows[0].id < rows[1].id));
    }

    #[test]
    fn act128_rank_config_maps_currency_thresholds_and_captured_rewards() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = crate::init(&data_dir);
        let tables = crate::configs::get();

        assert_eq!(tables.activity128_rank_currency_id(), Some(3206));
        assert_eq!(tables.activity128_rank(700), Some(7));

        let levels = tables.activity128_milestone_levels(2, 7).unwrap();
        let bonuses = levels
            .into_iter()
            .filter(|row| !row.bonus.is_empty())
            .map(|row| row.bonus.as_str())
            .collect::<Vec<_>>();
        assert_eq!(bonuses, vec!["1#120013#2", "1#110404#1"]);
    }
}
