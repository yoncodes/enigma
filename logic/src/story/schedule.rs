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

pub(super) fn is_active(story_id: i32, now: i32) -> bool {
    common::hero_story_schedule()
        .iter()
        .any(|row| row.story_id == story_id && now >= row.start_time && now <= row.end_time)
}
