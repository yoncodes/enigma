use sonettobuf::HeroStoryTime;

pub(super) fn times() -> Vec<HeroStoryTime> {
    common::hero_story_schedule()
        .iter()
        .map(|row| HeroStoryTime {
            story_id: Some(row.story_id),
            start_time: Some(row.start_time),
            end_time: Some(row.end_time),
            start_time_resident: Some(row.start_time_resident),
            end_time_resident: Some(row.end_time_resident),
        })
        .collect()
}
