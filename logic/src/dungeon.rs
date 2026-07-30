use crate::{
    error::AppError,
    reward::{self, AppliedRewards, RewardManager, RewardSet},
};
use database::{
    db::game::{dungeons, open_infos, stories},
    models::game::dungeons::UserDungeon,
};
use sonettobuf::{
    GetMapElementRecordReply, GetPointRewardReply, MapElementRecordInfo, MapElementReply, OpenInfo,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const SHARED_REWARD_POINT_CHAPTER_ID: i32 = 0;
const MAX_MAP_ELEMENT_RECORD_BYTES: usize = 64 * 1024;
const MAX_MAP_ELEMENT_DIALOGS: usize = 1024;

#[derive(Default)]
pub struct DungeonUnlock {
    pub changed: bool,
    pub episodes: Vec<EpisodeCompletion>,
    pub trails: TrailCompletion,
}

#[derive(Default)]
pub struct TrailCompletion {
    pub finished_element_ids: Vec<i32>,
    pub reward_points: BTreeMap<i32, i32>,
    pub rewards: AppliedRewards,
    pub material_changes: BTreeMap<i32, Vec<(u32, u32, i32)>>,
}

#[derive(Clone, Copy, Debug)]
pub struct DungeonManager {
    player_id: i64,
}

impl DungeonManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn unlock_stage(
        self,
        db: &SqlitePool,
        episode_id: i32,
    ) -> Result<DungeonUnlock, AppError> {
        config::configs::get()
            .episode
            .get(episode_id)
            .ok_or(AppError::InvalidRequest)?;
        self.unlock(db, [episode_id]).await
    }

    pub async fn unlock_chapter(
        self,
        db: &SqlitePool,
        chapter_id: i32,
    ) -> Result<DungeonUnlock, AppError> {
        let tables = config::configs::get();
        tables
            .chapter
            .get(chapter_id)
            .ok_or(AppError::InvalidRequest)?;
        let missing_heroes = chapter_missing_reward_heroes(chapter_id);
        if !missing_heroes.is_empty() {
            return Err(AppError::Custom(format!(
                "chapter {chapter_id} rewards unavailable heroes {missing_heroes:?}"
            )));
        }
        let targets = tables
            .episode
            .iter()
            .filter(|episode| episode.chapter_id == chapter_id)
            .map(|episode| episode.id)
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        self.unlock(db, targets).await
    }

    pub async fn claim_point_rewards(
        self,
        db: &SqlitePool,
        reward_ids: Vec<i32>,
    ) -> Result<PointRewardClaim, AppError> {
        if reward_ids.is_empty() {
            return Ok(PointRewardClaim::default());
        }
        let mut unique = HashSet::with_capacity(reward_ids.len());
        if reward_ids
            .iter()
            .any(|reward_id| *reward_id <= 0 || !unique.insert(*reward_id))
        {
            return Err(AppError::InvalidRequest);
        }

        let tables = config::configs::get();
        let mut tx = db.begin().await?;
        let points = dungeons::reward_point_in_transaction(
            &mut tx,
            self.player_id,
            SHARED_REWARD_POINT_CHAPTER_ID,
        )
        .await?;
        let mut reward_set = RewardSet::default();
        for reward_id in &reward_ids {
            let row = tables
                .chapter_point_reward
                .get(*reward_id)
                .filter(|row| row.reward_point_num > 0 && row.reward_point_num <= points)
                .ok_or(AppError::InvalidRequest)?;
            if !dungeons::claim_point_reward_in_transaction(
                &mut tx,
                self.player_id,
                SHARED_REWARD_POINT_CHAPTER_ID,
                *reward_id,
            )
            .await?
            {
                return Err(AppError::InvalidRequest);
            }
            reward_set.extend(reward::parse(&row.reward));
        }

        let material_changes = reward_set.material_changes();
        let rewards = RewardManager::new(self.player_id)
            .apply_in_transaction(&mut tx, db, reward_set)
            .await?;
        tx.commit().await?;
        Ok(PointRewardClaim {
            reply: GetPointRewardReply { id: reward_ids },
            rewards,
            material_changes,
        })
    }

    pub async fn complete_map_element(
        self,
        db: &SqlitePool,
        element_id: i32,
        dialog_ids: Vec<i32>,
        record: String,
    ) -> Result<MapElementCompletion, AppError> {
        if element_id <= 0
            || dialog_ids.len() > MAX_MAP_ELEMENT_DIALOGS
            || record.len() > MAX_MAP_ELEMENT_RECORD_BYTES
        {
            return Err(AppError::InvalidRequest);
        }
        let tables = config::configs::get();
        let element = tables
            .chapter_map_element
            .get(element_id)
            .ok_or(AppError::InvalidRequest)?;
        let reward_set = reward::parse(map_element_reward(tables, element)?);
        let material_changes = reward_set.material_changes();
        let mut tx = db.begin().await?;
        if !dungeons::complete_map_element_in_transaction(
            &mut tx,
            self.player_id,
            element_id,
            &record,
        )
        .await?
        {
            return Err(AppError::InvalidRequest);
        }
        let reward_point = if element.reward_point > 0 {
            Some((
                SHARED_REWARD_POINT_CHAPTER_ID,
                dungeons::add_reward_points_in_transaction(
                    &mut tx,
                    self.player_id,
                    SHARED_REWARD_POINT_CHAPTER_ID,
                    element.reward_point,
                )
                .await?,
            ))
        } else {
            None
        };
        let rewards = RewardManager::new(self.player_id)
            .apply_in_transaction(&mut tx, db, reward_set)
            .await?;
        tx.commit().await?;

        Ok(MapElementCompletion {
            reply: MapElementReply {
                element_id: Some(element_id),
                dialog_ids,
                record: Some(record),
            },
            rewards,
            material_changes,
            reward_point,
        })
    }

    pub async fn map_element_records(
        self,
        db: &SqlitePool,
        element_ids: Vec<i32>,
    ) -> Result<GetMapElementRecordReply, AppError> {
        let mut unique = HashSet::with_capacity(element_ids.len());
        if element_ids.len() > MAX_MAP_ELEMENT_DIALOGS
            || element_ids.iter().any(|id| *id <= 0 || !unique.insert(*id))
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(GetMapElementRecordReply {
            record_infos: dungeons::get_map_element_records(db, self.player_id, &element_ids)
                .await?
                .into_iter()
                .map(|(element_id, record)| MapElementRecordInfo {
                    element_id: Some(element_id),
                    record: Some(record),
                })
                .collect(),
        })
    }

    async fn unlock(
        self,
        db: &SqlitePool,
        targets: impl IntoIterator<Item = i32>,
    ) -> Result<DungeonUnlock, AppError> {
        let targets = targets.into_iter().collect::<Vec<_>>();
        let prerequisites = dungeons::prerequisite_episode_ids(targets.iter().copied())?;
        let episodes = config::configs::get()
            .tutorial_episodes()
            .map(|episode| episode.id)
            .chain(prerequisites)
            .collect();
        self.complete(db, episodes, targets).await
    }

    async fn complete(
        self,
        db: &SqlitePool,
        episodes: Vec<i32>,
        targets: Vec<i32>,
    ) -> Result<DungeonUnlock, AppError> {
        let mut tx = db.begin().await?;
        let mut completed = Vec::new();
        let mut finished_element_ids = Vec::new();
        let mut reward_points = BTreeMap::new();
        let mut trail_reward_set = RewardSet::default();
        let mut trail_material_changes = BTreeMap::new();
        for episode_id in episodes {
            complete_episode_trails_in_transaction(
                &mut tx,
                self.player_id,
                episode_id,
                &mut finished_element_ids,
                &mut reward_points,
                &mut trail_reward_set,
                &mut trail_material_changes,
            )
            .await?;
            let completion =
                complete_episode_in_transaction(&mut tx, self.player_id, episode_id).await?;
            if completion.changed {
                completed.push(completion);
            }
        }
        for episode_id in targets {
            complete_episode_trails_in_transaction(
                &mut tx,
                self.player_id,
                episode_id,
                &mut finished_element_ids,
                &mut reward_points,
                &mut trail_reward_set,
                &mut trail_material_changes,
            )
            .await?;
        }
        let rewards = RewardManager::new(self.player_id)
            .apply_in_transaction(&mut tx, db, trail_reward_set)
            .await?;
        tx.commit().await?;
        Ok(DungeonUnlock {
            changed: !completed.is_empty() || !finished_element_ids.is_empty(),
            episodes: completed,
            trails: TrailCompletion {
                finished_element_ids,
                reward_points,
                rewards,
                material_changes: trail_material_changes,
            },
        })
    }
}

pub fn chapter_missing_reward_heroes(chapter_id: i32) -> Vec<i32> {
    let tables = config::configs::get();
    let mut missing = BTreeSet::new();
    for episode in tables
        .episode
        .iter()
        .filter(|episode| episode.chapter_id == chapter_id)
    {
        let rewards =
            completion_rewards(episode, true, 0, configured_clear_star(episode), 1).rewards;
        missing.extend(
            rewards
                .heroes
                .into_iter()
                .map(|(hero_id, _)| hero_id)
                .filter(|hero_id| tables.character.get(*hero_id).is_none()),
        );
        for element_id in episode
            .element_list
            .split('#')
            .filter_map(|id| id.parse::<i32>().ok())
        {
            let Some(element) = tables.chapter_map_element.get(element_id) else {
                continue;
            };
            missing.extend(
                reward::parse(&element.reward)
                    .heroes
                    .into_iter()
                    .map(|(hero_id, _)| hero_id)
                    .filter(|hero_id| tables.character.get(*hero_id).is_none()),
            );
        }
    }
    missing.into_iter().collect()
}

#[derive(Default)]
pub struct PointRewardClaim {
    pub reply: GetPointRewardReply,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct MapElementCompletion {
    pub reply: MapElementReply,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub reward_point: Option<(i32, i32)>,
}

fn map_element_reward<'a>(
    tables: &'a config::GameDB,
    element: &'a config::chapter_map_element::ChapterMapElement,
) -> Result<&'a str, AppError> {
    let chapter = tables
        .chapter_map
        .get(element.map_id)
        .and_then(|map| tables.chapter.get(map.chapter_id))
        .ok_or(AppError::InvalidRequest)?;
    if chapter.act_id != 0
        && tables
            .activity
            .get(chapter.act_id)
            .is_some_and(|activity| activity.is_retro_acitivity == 2)
        && !element.permanent_reward.is_empty()
    {
        return Ok(&element.permanent_reward);
    }
    Ok(&element.reward)
}

async fn complete_episode_trails_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    episode_id: i32,
    finished_element_ids: &mut Vec<i32>,
    reward_points: &mut BTreeMap<i32, i32>,
    rewards: &mut RewardSet,
    material_changes: &mut BTreeMap<i32, Vec<(u32, u32, i32)>>,
) -> Result<(), AppError> {
    let tables = config::configs::get();
    let episode = tables
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    for element_id in episode
        .element_list
        .split('#')
        .filter_map(|id| id.parse::<i32>().ok())
    {
        if !dungeons::finish_element_in_transaction(tx, player_id, element_id).await? {
            continue;
        }
        let element = tables
            .chapter_map_element
            .get(element_id)
            .ok_or(AppError::InvalidRequest)?;
        let chapter_id = tables
            .chapter_map
            .get(element.map_id)
            .ok_or(AppError::InvalidRequest)?
            .chapter_id;
        finished_element_ids.push(element_id);
        let element_rewards = reward::parse(&element.reward);
        material_changes
            .entry(chapter_id)
            .or_default()
            .extend(element_rewards.material_changes());
        rewards.extend(element_rewards);
        if element.reward_point > 0 {
            // DungeonMapModel stores every Trail point total in its chapter-0 bucket.
            reward_points.insert(
                SHARED_REWARD_POINT_CHAPTER_ID,
                dungeons::add_reward_points_in_transaction(
                    tx,
                    player_id,
                    SHARED_REWARD_POINT_CHAPTER_ID,
                    element.reward_point,
                )
                .await?,
            );
        }
    }
    Ok(())
}

pub struct EpisodeCompletion {
    pub chapter_id: i32,
    pub changed: bool,
    pub dungeon: Option<UserDungeon>,
    pub finished_story_ids: Vec<i32>,
    pub open_infos: Vec<OpenInfo>,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub(crate) async fn complete_episode(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
) -> Result<EpisodeCompletion, AppError> {
    let mut tx = db.begin().await?;
    let completion = complete_episode_in_transaction(&mut tx, player_id, episode_id).await?;
    tx.commit().await?;
    Ok(completion)
}

async fn complete_episode_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    episode_id: i32,
) -> Result<EpisodeCompletion, AppError> {
    let episode = config::configs::get()
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let star = configured_clear_star(episode);
    let previous_star = dungeons::episode_star_in_transaction(tx, player_id, episode_id).await?;
    let repair_star =
        dungeons::claim_reward_repair_in_transaction(tx, player_id, episode_id).await?;
    let mut rewards = AppliedRewards::default();
    let mut material_changes = Vec::new();
    let mut dungeon = None;
    let mut finished_story_ids = Vec::new();
    let mut changed_open_infos = Vec::new();
    let mut changed = repair_star.is_some() || previous_star < star;
    if let Some(repair_star) = repair_star {
        let repaired_star = repair_star.max(star);
        if previous_star < repaired_star {
            dungeon = Some(
                dungeons::update_dungeon_progress_in_transaction(
                    tx,
                    player_id,
                    episode.chapter_id,
                    episode.id,
                    repaired_star,
                )
                .await?
                .0,
            );
        }
        let completion = completion_rewards(episode, true, 0, repaired_star, 1);
        material_changes = completion.rewards.material_changes();
        rewards = RewardManager::new(player_id)
            .apply_dungeon_in_transaction(tx, completion.rewards)
            .await?;
    } else if previous_star < star {
        dungeon = Some(
            dungeons::update_dungeon_progress_in_transaction(
                tx,
                player_id,
                episode.chapter_id,
                episode.id,
                star,
            )
            .await?
            .0,
        );
        let first_pass = previous_star == 0;
        let completion = completion_rewards(
            episode,
            first_pass,
            previous_star,
            star,
            i32::from(first_pass),
        );
        material_changes = completion.rewards.material_changes();
        rewards = RewardManager::new(player_id)
            .apply_dungeon_in_transaction(tx, completion.rewards)
            .await?;
    }
    if dungeon.is_some() || rewards.player_info_changed {
        changed_open_infos =
            open_infos::reconcile_progression_in_transaction(tx, player_id).await?;
    }
    for story_id in [episode.before_story, episode.after_story]
        .into_iter()
        .filter(|id| *id > 0)
    {
        if stories::finish_story_in_transaction(tx, player_id, story_id).await? {
            changed = true;
            finished_story_ids.push(story_id);
        }
    }
    Ok(EpisodeCompletion {
        chapter_id: episode.chapter_id,
        changed,
        dungeon,
        finished_story_ids,
        open_infos: changed_open_infos,
        rewards,
        material_changes,
    })
}

fn configured_clear_star(episode: &config::episode::Episode) -> i32 {
    let advanced = config::configs::get()
        .battle
        .get(episode.battle_id)
        .map(|battle| {
            battle
                .advanced_condition
                .split('|')
                .filter(|condition| !condition.is_empty())
                .count() as i32
        })
        .unwrap_or_default();
    1 + advanced
}

pub struct CompletionRewards {
    pub rewards: RewardSet,
    pub player_exp: i32,
    pub first_bonus: Vec<(u32, u32, i32)>,
    pub normal_bonus: Vec<(u32, u32, i32)>,
    pub advanced_bonus: Vec<(u32, u32, i32)>,
}

pub fn completion_rewards(
    episode: &config::episode::Episode,
    first_pass: bool,
    previous_star: i32,
    star: i32,
    multiplier: i32,
) -> CompletionRewards {
    let tables = config::configs::get();
    let cost = episode_cost_value(episode);
    let mut normal_rewards = reward::parse_bonus_with_cost(episode.bonus, cost);
    if tables.is_breakthrough_episode(episode) {
        normal_rewards.extend(reward::parse(&episode.reward_list));
    }
    normal_rewards.scale(multiplier);
    let first_rewards = if first_pass {
        reward::parse_bonus_with_cost(episode.first_bonus, cost)
    } else {
        Default::default()
    };
    let advanced_rewards = if previous_star < 2 && star >= 2 {
        reward::parse_bonus_with_cost(episode.advanced_bonus, cost)
    } else {
        Default::default()
    };
    let normal_bonus = normal_rewards.material_changes();
    let first_bonus = first_rewards.material_changes();
    let advanced_bonus = advanced_rewards.material_changes();
    let mut rewards = normal_rewards;
    rewards.extend(first_rewards);
    rewards.extend(advanced_rewards);
    if first_pass && tables.initial_tutorial_final_episode() == Some(episode.id) {
        let (packages, buildings) = tables.initial_room_rewards();
        rewards
            .block_packages
            .extend(packages.into_iter().map(|id| (id, 1)));
        rewards
            .room_buildings
            .extend(buildings.into_iter().map(|id| (id, 1)));
    }
    CompletionRewards {
        rewards,
        player_exp: 0,
        first_bonus,
        normal_bonus,
        advanced_bonus,
    }
}

pub fn episode_cost_value(episode: &config::episode::Episode) -> i32 {
    episode
        .cost
        .split('|')
        .find_map(|part| part.rsplit('#').next()?.parse::<i32>().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod test;
