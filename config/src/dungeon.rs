use crate::{GameDB, episode::Episode};

pub struct EpisodeRewards {
    pub reward: i32,
    pub bonus: i32,
    pub first_bonus: i32,
    pub advanced_bonus: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum ChapterKind {
    Story = 1,
    Hard = 2,
    Tutorial = 3,
    Gold = 4,
    Experience = 5,
    Equipment = 6,
    Breakthrough = 7,
    SpecialEquipment = 8,
    Buildings = 13,
}

impl ChapterKind {
    const RESOURCES: [Self; 6] = [
        Self::Gold,
        Self::Experience,
        Self::Equipment,
        Self::Breakthrough,
        Self::SpecialEquipment,
        Self::Buildings,
    ];

    fn is_story_family(value: i32) -> bool {
        value == Self::Story as i32 || value == Self::Hard as i32
    }
}

impl GameDB {
    pub fn episode_rewards(&self, episode: &Episode) -> EpisodeRewards {
        let activity = self
            .chapter
            .get(episode.chapter_id)
            .and_then(|chapter| self.activity.get(chapter.act_id));
        match activity.map(|activity| activity.is_retro_acitivity) {
            Some(2) => EpisodeRewards {
                reward: episode.permanent_reward,
                bonus: episode.permanent_bonus,
                first_bonus: episode.permanent_first_bonus,
                advanced_bonus: episode.permanent_advanced_bonus,
            },
            Some(1) => EpisodeRewards {
                reward: episode.retro_reward,
                bonus: episode.retro_bonus,
                first_bonus: episode.retro_first_bonus,
                advanced_bonus: episode.retro_advanced_bonus,
            },
            _ => EpisodeRewards {
                reward: episode.reward,
                bonus: episode.bonus,
                first_bonus: episode.first_bonus,
                advanced_bonus: episode.advanced_bonus,
            },
        }
    }

    pub fn initial_tutorial_final_episode(&self) -> Option<i32> {
        let guide_id = self
            .guide
            .iter()
            .find(|guide| guide.is_online != 0 && guide.trigger == "PlayerLv#1")?
            .id;

        self.guide_step
            .iter()
            .filter(|step| step.id == guide_id)
            .flat_map(|step| step.action.split('|'))
            .filter_map(|action| {
                let mut fields = action.split('#');
                (fields.next() == Some("102"))
                    .then(|| fields.next()?.parse().ok())
                    .flatten()
            })
            .next_back()
    }

    pub fn story_episodes(&self) -> impl Iterator<Item = &Episode> {
        self.episodes_in_chapter_kind(ChapterKind::Story)
    }

    pub fn tutorial_episodes(&self) -> impl Iterator<Item = &Episode> {
        self.episodes_in_chapter_kind(ChapterKind::Tutorial)
    }

    pub fn chapter_family_ids(&self, chapter_id: i32) -> Option<Vec<i32>> {
        let chapter = self.chapter.get(chapter_id)?;
        Some(
            self.chapter
                .iter()
                .filter(|candidate| {
                    if ChapterKind::is_story_family(chapter.r#type) {
                        ChapterKind::is_story_family(candidate.r#type)
                            && candidate.name == chapter.name
                    } else {
                        candidate.id == chapter.id
                    }
                })
                .map(|candidate| candidate.id)
                .collect(),
        )
    }

    pub fn episode_family_ids(&self, episode_id: i32) -> Option<Vec<i32>> {
        let episode = self.episode.get(episode_id)?;
        let chapter = self.chapter.get(episode.chapter_id)?;
        if !ChapterKind::is_story_family(chapter.r#type) {
            return Some(vec![episode.id]);
        }
        let chapter_ids = self.chapter_family_ids(episode.chapter_id)?;
        Some(
            self.episode
                .iter()
                .filter(|candidate| {
                    chapter_ids.contains(&candidate.chapter_id) && candidate.name == episode.name
                })
                .map(|candidate| candidate.id)
                .collect(),
        )
    }

    pub fn resource_episodes(&self) -> impl Iterator<Item = &Episode> {
        self.episode.iter().filter(|episode| {
            self.chapter.get(episode.chapter_id).is_some_and(|chapter| {
                ChapterKind::RESOURCES
                    .iter()
                    .any(|kind| chapter.r#type == *kind as i32)
            })
        })
    }

    pub fn is_breakthrough_episode(&self, episode: &Episode) -> bool {
        self.chapter
            .get(episode.chapter_id)
            .is_some_and(|chapter| chapter.r#type == ChapterKind::Breakthrough as i32)
    }

    fn episodes_in_chapter_kind(&self, kind: ChapterKind) -> impl Iterator<Item = &Episode> {
        self.episode.iter().filter(move |episode| {
            self.chapter
                .get(episode.chapter_id)
                .is_some_and(|chapter| chapter.r#type == kind as i32)
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn story_and_tutorial_episode_queries_do_not_overlap() {
        crate::init_test_config();
        let story = crate::get()
            .story_episodes()
            .map(|episode| episode.id)
            .collect::<std::collections::HashSet<_>>();
        let tutorials = crate::get()
            .tutorial_episodes()
            .map(|episode| episode.id)
            .collect::<Vec<_>>();
        let resources = crate::get()
            .resource_episodes()
            .map(|episode| episode.id)
            .collect::<Vec<_>>();

        assert!(story.contains(&10101));
        assert!(story.contains(&10102));
        assert!(tutorials.contains(&10001));
        assert!(resources.contains(&40101));
        assert!(tutorials.iter().all(|episode| !story.contains(episode)));
        assert!(resources.iter().all(|episode| !story.contains(episode)));
        assert_eq!(crate::get().initial_tutorial_final_episode(), Some(10003));
        assert_eq!(crate::get().chapter_family_ids(103), Some(vec![103, 203]));
        assert_eq!(
            crate::get().episode_family_ids(10303),
            Some(vec![10303, 20301])
        );
        assert_eq!(crate::get().chapter_family_ids(401), Some(vec![401]));
        assert_eq!(crate::get().episode_family_ids(40106), Some(vec![40106]));
        assert_eq!(crate::get().episode_family_ids(10002), Some(vec![10002]));
    }
}
