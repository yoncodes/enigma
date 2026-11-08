use sonettobuf::HeroInfo;
use sqlx::SqlitePool;

use crate::{error::AppError, hero::HeroManager, reward::HeroGain};

pub enum HeroRewardEvent {
    HeroChanged(Vec<HeroInfo>),
    PlayerInfoChanged,
    PlayerCardProfileChanged,
    PlayerCardHeroChanged,
    HeroGained(HeroGain),
}

pub async fn hero_reward_events(
    db: &SqlitePool,
    player_id: i64,
    hero_ids: Vec<i32>,
    gains: Vec<HeroGain>,
) -> Result<Vec<HeroRewardEvent>, AppError> {
    if hero_ids.len() != gains.len() {
        return Err(AppError::Custom(
            "hero reward projection is missing grant metadata".into(),
        ));
    }

    let final_state = HeroManager::new(player_id).snapshots(db, hero_ids).await?;
    let mut events = Vec::new();
    if gains.iter().any(|gain| gain.is_first) {
        let mut initial_state = final_state.clone();
        for (hero, gain) in initial_state.iter_mut().zip(&gains) {
            if gain.is_first {
                hero.voice
                    .retain(|voice_id| !gain.initial_voice_ids.contains(voice_id));
            }
        }
        events.push(HeroRewardEvent::HeroChanged(initial_state));
        events.push(HeroRewardEvent::PlayerInfoChanged);
        events.push(HeroRewardEvent::PlayerCardProfileChanged);
        events.push(HeroRewardEvent::PlayerCardHeroChanged);
        events.push(HeroRewardEvent::HeroChanged(final_state));
    } else {
        events.push(HeroRewardEvent::HeroChanged(final_state));
        events.push(HeroRewardEvent::PlayerCardHeroChanged);
    }
    events.extend(gains.into_iter().map(HeroRewardEvent::HeroGained));
    Ok(events)
}
