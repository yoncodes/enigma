use crate::{GameDB, episode::Episode};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum ChapterKind {
    Story = 1,
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
}

impl GameDB {
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
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = crate::init(data_dir.to_str().unwrap());
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
    }
}
