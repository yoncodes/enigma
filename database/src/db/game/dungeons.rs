use crate::{
    db::game::open_infos,
    models::game::dungeons::{
        DungeonLastHeroGroup, RewardPointInfo, UserChapterTypeNum, UserDungeon,
    },
    models::game::heros::{HeroModel, UserHeroModel},
};

use anyhow::{Context, Result, ensure};
use common::time::ServerTime;
use config::configs;
use sonettobuf::OpenInfo;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};

pub async fn get_user_dungeons(pool: &SqlitePool, user_id: i64) -> Result<Vec<UserDungeon>> {
    let dungeons = sqlx::query_as::<_, UserDungeon>(
        "SELECT * FROM user_dungeons WHERE user_id = ? ORDER BY chapter_id, episode_id
",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(dungeons)
}

pub async fn get_user_dungeon_state(
    pool: &SqlitePool,
    user_id: i64,
    chapter_id: i32,
    episode_id: i32,
) -> Result<UserDungeon> {
    let dungeon = sqlx::query_as::<_, UserDungeon>(
        "SELECT *
         FROM user_dungeons
         WHERE user_id = ? AND chapter_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(chapter_id)
    .bind(episode_id)
    .fetch_optional(pool)
    .await?;

    if let Some(dungeon) = dungeon {
        return Ok(dungeon);
    }

    let episode = configs::get()
        .episode
        .get(episode_id)
        .with_context(|| format!("missing episode config {episode_id}"))?;
    ensure!(
        episode.chapter_id == chapter_id,
        "episode {episode_id} does not belong to chapter {chapter_id}"
    );
    Ok(UserDungeon {
        id: 0,
        user_id,
        chapter_id,
        episode_id,
        star: 0,
        challenge_count: 0,
        has_record: false,
        left_return_all_num: 1,
        today_pass_num: 0,
        today_total_num: episode.day_num,
        created_at: 0,
        updated_at: 0,
    })
}

pub async fn episode_star(pool: &SqlitePool, user_id: i64, episode_id: i32) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(star), 0) FROM user_dungeons WHERE user_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_one(pool)
    .await?)
}

pub async fn episode_star_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    episode_id: i32,
) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(star), 0) FROM user_dungeons WHERE user_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn claim_reward_repair_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    episode_id: i32,
) -> Result<Option<i32>> {
    let repaired_at = ServerTime::now_ms();
    Ok(sqlx::query_scalar(
        "UPDATE user_dungeon_reward_repairs
         SET repaired_at = ?
         WHERE user_id = ? AND episode_id = ? AND repaired_at IS NULL
         RETURNING star",
    )
    .bind(repaired_at)
    .bind(user_id)
    .bind(episode_id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub async fn get_dungeon_last_hero_groups(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<DungeonLastHeroGroup>> {
    // Get all last hero groups with their chapter IDs
    let rows: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT chapter_id, hero_group_id FROM dungeon_last_hero_groups WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for (chapter_id, hero_group_id) in rows {
        // Get the hero group info
        if let Some(group_info) =
            crate::db::game::hero_groups::get_hero_group(pool, user_id, hero_group_id).await?
        {
            result.push(DungeonLastHeroGroup {
                chapter_id,
                hero_group_info: group_info,
            });
        }
    }

    Ok(result)
}

pub async fn get_unlocked_maps(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let maps = sqlx::query_scalar("SELECT map_id FROM user_dungeon_maps WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(maps)
}

pub async fn get_elements(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let elements = sqlx::query_scalar(
        "SELECT element_id FROM user_dungeon_elements WHERE user_id = ? AND is_finished = 0",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(elements)
}

pub async fn get_finished_elements(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let elements = sqlx::query_scalar(
        "SELECT element_id FROM user_dungeon_elements WHERE user_id = ? AND is_finished = 1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(elements)
}

pub async fn finish_element_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    element_id: i32,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO user_dungeon_elements
            (user_id, element_id, is_finished, puzzle_progress, puzzle_updated_at)
         VALUES (?, ?, 1, '', ?)
         ON CONFLICT(user_id, element_id) DO UPDATE SET
            is_finished = 1,
            puzzle_progress = '',
            puzzle_updated_at = excluded.puzzle_updated_at
         WHERE is_finished = 0",
    )
    .bind(user_id)
    .bind(element_id)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() != 0)
}

pub async fn complete_map_element_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    element_id: i32,
    record: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_dungeon_elements
         SET is_finished = 1, element_record = ?, puzzle_updated_at = ?
         WHERE user_id = ? AND element_id = ? AND is_finished = 0",
    )
    .bind(record)
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(element_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn get_map_element_records(
    pool: &SqlitePool,
    user_id: i64,
    element_ids: &[i32],
) -> Result<Vec<(i32, String)>> {
    let mut records = Vec::new();
    for element_id in element_ids {
        if let Some(record) = sqlx::query_scalar(
            "SELECT element_record FROM user_dungeon_elements
             WHERE user_id = ? AND element_id = ? AND is_finished = 1",
        )
        .bind(user_id)
        .bind(element_id)
        .fetch_optional(pool)
        .await?
        {
            records.push((*element_id, record));
        }
    }
    Ok(records)
}

pub async fn add_reward_points_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    chapter_id: i32,
    amount: i32,
) -> Result<i32> {
    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_dungeon_reward_points
            (user_id, chapter_id, reward_point, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, chapter_id) DO UPDATE SET
            reward_point = reward_point + excluded.reward_point,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(chapter_id)
    .bind(amount)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(sqlx::query_scalar(
        "SELECT reward_point FROM user_dungeon_reward_points
         WHERE user_id = ? AND chapter_id = ?",
    )
    .bind(user_id)
    .bind(chapter_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn get_reward_points(pool: &SqlitePool, user_id: i64) -> Result<Vec<RewardPointInfo>> {
    let points: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT chapter_id, reward_point FROM user_dungeon_reward_points WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for (chapter_id, reward_point) in points {
        let claimed_rewards = sqlx::query_scalar(
            "SELECT point_reward_id FROM user_dungeon_claimed_rewards
             WHERE user_id = ? AND chapter_id = ?",
        )
        .bind(user_id)
        .bind(chapter_id)
        .fetch_all(pool)
        .await?;

        result.push(RewardPointInfo {
            chapter_id,
            reward_point,
            has_get_point_reward_ids: claimed_rewards,
        });
    }

    Ok(result)
}

pub async fn reward_point_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    chapter_id: i32,
) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "SELECT reward_point FROM user_dungeon_reward_points
         WHERE user_id = ? AND chapter_id = ?",
    )
    .bind(user_id)
    .bind(chapter_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default())
}

pub async fn claim_point_reward_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    chapter_id: i32,
    reward_id: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "INSERT INTO user_dungeon_claimed_rewards
            (user_id, chapter_id, point_reward_id)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, chapter_id, point_reward_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(chapter_id)
    .bind(reward_id)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        != 0)
}

pub async fn get_equip_sp_chapters(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let chapters = sqlx::query_scalar(
        "SELECT chapter_id FROM user_dungeon_equip_sp_chapters WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(chapters)
}

pub async fn get_chapter_type_nums(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<UserChapterTypeNum>> {
    let nums = sqlx::query_as::<_, UserChapterTypeNum>(
        "SELECT chapter_type, today_pass_num, today_total_num
         FROM user_chapter_type_nums WHERE user_id = ? ORDER BY chapter_type",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(nums)
}

pub async fn get_finished_puzzles(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let puzzles =
        sqlx::query_scalar("SELECT puzzle_id FROM user_dungeon_finished_puzzles WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(puzzles)
}

pub async fn get_puzzle_progress(
    pool: &SqlitePool,
    user_id: i64,
    element_id: i32,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT puzzle_progress FROM user_dungeon_elements
         WHERE user_id = ? AND element_id = ?",
    )
    .bind(user_id)
    .bind(element_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn save_puzzle_progress(
    pool: &SqlitePool,
    user_id: i64,
    element_id: i32,
    progress: &str,
) -> Result<bool> {
    let owns_element: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM user_dungeon_elements
             WHERE user_id = ? AND element_id = ? AND is_finished = 0
         )",
    )
    .bind(user_id)
    .bind(element_id)
    .fetch_one(pool)
    .await?;
    if !owns_element {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE user_dungeon_elements
         SET puzzle_progress = ?, puzzle_updated_at = ?
         WHERE user_id = ? AND element_id = ?",
    )
    .bind(progress)
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(element_id)
    .execute(pool)
    .await?;
    Ok(true)
}

pub async fn finish_puzzle(pool: &SqlitePool, user_id: i64, element_id: i32) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE user_dungeon_elements
         SET is_finished = 1, puzzle_progress = '', puzzle_updated_at = ?
         WHERE user_id = ? AND element_id = ?",
    )
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(element_id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query(
        "INSERT OR IGNORE INTO user_dungeon_finished_puzzles (user_id, puzzle_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(element_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn reconcile_map_progression(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(Vec<i32>, Vec<i32>)> {
    let game_data = configs::get();
    let completed = get_user_dungeons(pool, user_id)
        .await?
        .into_iter()
        .filter(|dungeon| dungeon.star > 0)
        .map(|dungeon| dungeon.episode_id)
        .collect::<HashSet<_>>();
    let completed_conditions = completed
        .iter()
        .copied()
        .chain(completed.iter().filter_map(|episode_id| {
            game_data
                .episode
                .get(*episode_id)
                .map(|episode| episode.chain_episode)
                .filter(|chain_episode| *chain_episode > 0)
        }))
        .collect::<HashSet<_>>();
    let finished_stories = crate::db::game::stories::get_finished_stories(pool, user_id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let chain_episodes = game_data
        .episode
        .iter()
        .filter(|episode| episode.chain_episode > 0)
        .map(|episode| (episode.chain_episode, episode.id))
        .collect::<HashMap<_, _>>();
    let mut maps = get_unlocked_maps(pool, user_id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let mut added_maps = Vec::new();

    for map in game_data.chapter_map.iter() {
        let unlocked = if let Some(episode_id) = episode_finish(&map.unlock_condition) {
            completed_conditions.contains(&episode_id)
        } else if map.unlock_condition.is_empty() {
            game_data
                .chapter
                .get(map.chapter_id)
                .and_then(|chapter| game_data.episode.get(chapter.episode_id))
                .is_some_and(|episode| {
                    episode_is_unlocked(
                        game_data,
                        &chain_episodes,
                        episode,
                        &completed,
                        &finished_stories,
                    )
                })
        } else {
            false
        };
        if unlocked && maps.insert(map.id) {
            sqlx::query("INSERT INTO user_dungeon_maps (user_id, map_id) VALUES (?, ?)")
                .bind(user_id)
                .bind(map.id)
                .execute(pool)
                .await?;
            added_maps.push(map.id);
        }
    }

    let finished_elements = get_finished_elements(pool, user_id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let mut elements = get_elements(pool, user_id)
        .await?
        .into_iter()
        .chain(finished_elements.iter().copied())
        .collect::<HashSet<_>>();
    let mut added_elements = Vec::new();
    for element in game_data.chapter_map_element.iter() {
        let unlocked = maps.contains(&element.map_id)
            && element_condition_met(
                &element.condition,
                &completed_conditions,
                &finished_elements,
            );
        if unlocked && elements.insert(element.id) {
            sqlx::query(
                "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
                 VALUES (?, ?, 0)",
            )
            .bind(user_id)
            .bind(element.id)
            .execute(pool)
            .await?;
            added_elements.push(element.id);
        }
    }

    Ok((added_maps, added_elements))
}

pub async fn can_start_episode(
    pool: &SqlitePool,
    user_id: i64,
    chapter_id: i32,
    episode_id: i32,
) -> Result<bool> {
    let game_data = configs::get();
    let Some(episode) = game_data.episode.get(episode_id) else {
        return Ok(false);
    };
    let Some(chapter) = game_data.chapter.get(chapter_id) else {
        return Ok(false);
    };
    if episode.chapter_id != chapter_id {
        return Ok(false);
    }

    let completed = get_user_dungeons(pool, user_id)
        .await?
        .into_iter()
        .filter(|dungeon| dungeon.star > 0)
        .map(|dungeon| dungeon.episode_id)
        .collect::<HashSet<_>>();
    let finished_stories = crate::db::game::stories::get_finished_stories(pool, user_id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let chain_episodes = game_data
        .episode
        .iter()
        .filter(|episode| episode.chain_episode > 0)
        .map(|episode| (episode.chain_episode, episode.id))
        .collect::<HashMap<_, _>>();
    if !episode_is_unlocked(
        game_data,
        &chain_episodes,
        episode,
        &completed,
        &finished_stories,
    ) || (episode.unlock_episode > 0 && !completed.contains(&episode.unlock_episode))
    {
        return Ok(false);
    }
    let active_elements = get_elements(pool, user_id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    if episode_element_ids(episode).any(|element_id| active_elements.contains(&element_id)) {
        return Ok(false);
    }

    if let Some(gate) = game_data
        .open
        .iter()
        .find(|open| open.show_in_episode != 0 && open.name == chapter.name)
    {
        return Ok(super::open_infos::get_open_info(pool, user_id, gate.id)
            .await?
            .is_open);
    }

    Ok(true)
}

fn episode_element_ids(episode: &config::episode::Episode) -> impl Iterator<Item = i32> + '_ {
    episode
        .element_list
        .split('#')
        .filter_map(|id| id.parse().ok())
}

fn episode_finish(condition: &str) -> Option<i32> {
    condition.strip_prefix("EpisodeFinish=")?.parse().ok()
}

fn element_condition_met(
    condition: &str,
    completed_episodes: &HashSet<i32>,
    finished_elements: &HashSet<i32>,
) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }
    let condition = strip_outer_parentheses(condition);
    if let Some(parts) = split_condition(condition, " or ") {
        return parts
            .into_iter()
            .any(|part| element_condition_met(part, completed_episodes, finished_elements));
    }
    if let Some(parts) = split_condition(condition, " and ") {
        return parts
            .into_iter()
            .all(|part| element_condition_met(part, completed_episodes, finished_elements));
    }
    episode_finish(condition).is_some_and(|id| completed_episodes.contains(&id))
        || condition
            .strip_prefix("ChapterMapElement=")
            .and_then(|id| id.parse().ok())
            .is_some_and(|id| finished_elements.contains(&id))
}

fn strip_outer_parentheses(mut condition: &str) -> &str {
    while condition.starts_with('(')
        && condition.ends_with(')')
        && matching_closing_parenthesis(condition) == Some(condition.len() - 1)
    {
        condition = condition[1..condition.len() - 1].trim();
    }
    condition
}

fn matching_closing_parenthesis(condition: &str) -> Option<usize> {
    let mut depth = 0;
    for (index, byte) in condition.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_condition<'a>(condition: &'a str, operator: &str) -> Option<Vec<&'a str>> {
    let bytes = condition.as_bytes();
    let operator = operator.as_bytes();
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    let mut index = 0;
    while index + operator.len() <= bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ if depth == 0 && &bytes[index..index + operator.len()] == operator => {
                parts.push(&condition[start..index]);
                index += operator.len();
                start = index;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(&condition[start..]);
    Some(parts)
}

fn episode_is_unlocked(
    game_data: &config::GameDB,
    chain_episodes: &HashMap<i32, i32>,
    episode: &config::episode::Episode,
    completed: &HashSet<i32>,
    finished_stories: &HashSet<i32>,
) -> bool {
    if episode.pre_episode == 0 {
        return true;
    }

    let effective = effective_prerequisite(chain_episodes, episode);
    [effective, episode.pre_episode]
        .into_iter()
        .any(|episode_id| {
            completed.contains(&episode_id)
                && game_data
                    .episode
                    .get(episode_id)
                    .is_none_or(|prerequisite| {
                        let story_id = effective_after_story(game_data, prerequisite);
                        story_id == 0 || finished_stories.contains(&story_id)
                    })
        })
}

pub async fn update_dungeon_progress(
    pool: &SqlitePool,
    user_id: i64,
    chapter_id: i32,
    episode_id: i32,
    stars_earned: i32,
) -> Result<(UserDungeon, Vec<UserChapterTypeNum>, Vec<OpenInfo>)> {
    let mut tx = pool.begin().await?;
    let (dungeon, chapter_type_nums) = update_dungeon_progress_in_transaction(
        &mut tx,
        user_id,
        chapter_id,
        episode_id,
        stars_earned,
    )
    .await?;
    tx.commit().await?;
    let open_infos = open_infos::reconcile_progression(pool, user_id).await?;
    Ok((dungeon, chapter_type_nums, open_infos))
}

pub async fn update_dungeon_progress_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    chapter_id: i32,
    episode_id: i32,
    stars_earned: i32,
) -> Result<(UserDungeon, Vec<UserChapterTypeNum>)> {
    let game_data = configs::get();
    let chapter = game_data
        .chapter
        .get(chapter_id)
        .with_context(|| format!("missing chapter config {chapter_id}"))?;
    let chapter_type = chapter.r#type;
    let episode = game_data
        .episode
        .get(episode_id)
        .with_context(|| format!("missing episode config {episode_id}"))?;
    ensure!(
        episode.chapter_id == chapter_id,
        "episode {episode_id} does not belong to chapter {chapter_id}"
    );
    let episode_daily_limit = episode.day_num;
    let chapter_daily_limit = game_data
        .r#const
        .get(78)
        .map(|value| chapter_type_daily_limit(&value.value, chapter_type))
        .unwrap_or_default();
    let challenge_increment = i32::from(!chapter.challenge_count_limit.is_empty());
    let daily_increment = i32::from(episode_daily_limit > 0);
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        r#"
        INSERT INTO user_dungeons
        (user_id, chapter_id, episode_id, star, challenge_count, has_record,
         left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 0, 1, ?, ?, ?, ?)
        ON CONFLICT(user_id, chapter_id, episode_id) DO UPDATE SET
            star = CASE WHEN excluded.star > star THEN excluded.star ELSE star END,
            challenge_count = challenge_count + excluded.challenge_count,
            today_pass_num = today_pass_num + excluded.today_pass_num,
            today_total_num = CASE
                WHEN excluded.today_total_num > 0 THEN excluded.today_total_num
                ELSE today_total_num
            END,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(user_id)
    .bind(chapter_id)
    .bind(episode_id)
    .bind(stars_earned)
    .bind(challenge_increment)
    .bind(daily_increment)
    .bind(episode_daily_limit)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    if chapter_daily_limit > 0 {
        sqlx::query(
            r#"
            INSERT INTO user_chapter_type_nums
            (user_id, chapter_type, today_pass_num, today_total_num, last_reset_date)
            VALUES (?, ?, 1, ?, ?)
            ON CONFLICT(user_id, chapter_type) DO UPDATE SET
                today_pass_num = today_pass_num + 1,
                today_total_num = excluded.today_total_num
            "#,
        )
        .bind(user_id)
        .bind(chapter_type)
        .bind(chapter_daily_limit)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }

    let dungeon = sqlx::query_as::<_, UserDungeon>(
        "SELECT * FROM user_dungeons
         WHERE user_id = ? AND chapter_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(chapter_id)
    .bind(episode_id)
    .fetch_one(&mut **tx)
    .await?;
    let chapter_type_nums = sqlx::query_as::<_, UserChapterTypeNum>(
        "SELECT chapter_type, today_pass_num, today_total_num
         FROM user_chapter_type_nums WHERE user_id = ? ORDER BY chapter_type",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok((dungeon, chapter_type_nums))
}

fn chapter_type_daily_limit(value: &str, chapter_type: i32) -> i32 {
    value
        .split('|')
        .find_map(|entry| {
            let (kind, count) = entry.split_once('#')?;
            (kind.parse::<i32>().ok()? == chapter_type).then(|| count.parse().unwrap_or_default())
        })
        .unwrap_or_default()
}

pub fn prerequisite_episode_ids(targets: impl IntoIterator<Item = i32>) -> Result<Vec<i32>> {
    let game_data = configs::get();
    let chain_episodes = game_data
        .episode
        .iter()
        .filter(|episode| episode.chain_episode > 0)
        .map(|episode| (episode.chain_episode, episode.id))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut episode_ids = Vec::new();
    for target in targets {
        collect_prerequisites(
            game_data,
            &chain_episodes,
            target,
            &mut seen,
            &mut episode_ids,
        )?;
    }
    Ok(episode_ids)
}

fn collect_prerequisites(
    game_data: &config::GameDB,
    chain_episodes: &HashMap<i32, i32>,
    episode_id: i32,
    seen: &mut HashSet<i32>,
    ordered: &mut Vec<i32>,
) -> Result<()> {
    let episode = game_data
        .episode
        .get(episode_id)
        .with_context(|| format!("missing episode config {episode_id}"))?;
    for prerequisite in [
        effective_prerequisite(chain_episodes, episode),
        episode.unlock_episode,
    ] {
        if prerequisite == 0 || !seen.insert(prerequisite) {
            continue;
        }
        collect_prerequisites(game_data, chain_episodes, prerequisite, seen, ordered)?;
        ordered.push(prerequisite);
    }
    Ok(())
}

fn effective_prerequisite(
    chain_episodes: &HashMap<i32, i32>,
    episode: &config::episode::Episode,
) -> i32 {
    if episode.pre_episode_id > 0 {
        episode.pre_episode_id
    } else {
        chain_episodes
            .get(&episode.pre_episode)
            .copied()
            .unwrap_or(episode.pre_episode)
    }
}

fn effective_after_story(game_data: &config::GameDB, episode: &config::episode::Episode) -> i32 {
    game_data
        .episode
        .get(episode.chain_episode)
        .map(|chain| chain.after_story)
        .unwrap_or(episode.after_story)
}

pub async fn load_dungeon_record(
    pool: &SqlitePool,
    user_id: i64,
    episode_id: i32,
) -> Result<Option<sonettobuf::FightGroupRecord>> {
    let hero = UserHeroModel::new(user_id, pool.clone());

    #[derive(sqlx::FromRow)]
    struct RecordRow {
        record_round: i32,
        hero_list: String,
        sub_hero_list: String,
        cloth_id: i32,
        equips: String,
        version: i32,
    }

    let row: Option<RecordRow> = sqlx::query_as(
        r#"
        SELECT record_round, hero_list, sub_hero_list, cloth_id, equips, version
        FROM dungeon_records
        WHERE user_id = ? AND episode_id = ?
        "#,
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let hero_list = load_fight_hero_records(&hero, &row.hero_list).await?;
    let sub_hero_list = load_fight_hero_records(&hero, &row.sub_hero_list).await?;

    // Parse equipment data and filter zeros
    let all_equips: Vec<sonettobuf::FightEquipRecord> = serde_json::from_str(&row.equips)?;
    let equips: Vec<sonettobuf::FightEquipRecord> = all_equips
        .into_iter()
        .filter(|e| e.hero_uid.unwrap_or(0) != 0) // Filter zero hero UIDs
        .collect();

    Ok(Some(sonettobuf::FightGroupRecord {
        hero_list,
        sub_hero_list,
        cloth_id: Some(row.cloth_id),
        equips,
        trial_hero_list: vec![],
        activity104_equips: vec![],
        ex_infos: vec![],
        version: Some(row.version),
        assist_user_id: Some(0),
        assist_hero_uid: Some(0),
        record_round: Some(row.record_round),
        assist_boss_id: Some(0),
    }))
}

async fn load_fight_hero_records(
    hero: &UserHeroModel,
    encoded: &str,
) -> Result<Vec<sonettobuf::FightHeroRecord>> {
    if let Ok(records) = serde_json::from_str(encoded) {
        return Ok(records);
    }

    let hero_uids = serde_json::from_str::<Vec<i64>>(encoded)?;
    Ok(build_fight_hero_records(hero, &hero_uids).await)
}

async fn build_fight_hero_records(
    hero: &UserHeroModel,
    hero_uids: &[i64],
) -> Vec<sonettobuf::FightHeroRecord> {
    let mut records = Vec::new();
    for &hero_uid in hero_uids.iter().filter(|uid| **uid != 0) {
        if let Ok(hero_data) = hero.get_uid(hero_uid).await {
            records.push(sonettobuf::FightHeroRecord {
                hero_uid: Some(hero_data.record.uid),
                hero_id: Some(hero_data.record.hero_id),
                level: Some(hero_data.record.level),
                skin: Some(hero_data.record.skin),
            });
        }
    }
    records
}

#[derive(Debug, Clone)]
pub struct PreparedDungeonRecord {
    hero_list: String,
    sub_hero_list: String,
    cloth_id: i32,
    equips: String,
    version: i32,
    oper_records: String,
}

pub async fn prepare_dungeon_record(
    pool: &SqlitePool,
    user_id: i64,
    version: i32,
    fight_group: &sonettobuf::FightGroup,
    oper_records: &[sonettobuf::FightRoundOperRecord],
) -> Result<PreparedDungeonRecord> {
    let hero = UserHeroModel::new(user_id, pool.clone());
    let hero_list =
        serde_json::to_string(&build_fight_hero_records(&hero, &fight_group.hero_list).await)?;
    let sub_hero_list =
        serde_json::to_string(&build_fight_hero_records(&hero, &fight_group.sub_hero_list).await)?;
    let equips =
        super::equipment::build_equip_records(pool, user_id, &Some(fight_group.clone())).await?;

    Ok(PreparedDungeonRecord {
        hero_list,
        sub_hero_list,
        cloth_id: fight_group.cloth_id.unwrap_or(1),
        equips: serde_json::to_string(&equips)?,
        version,
        oper_records: serde_json::to_string(oper_records)?,
    })
}

pub async fn save_prepared_dungeon_record_if_faster_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    episode_id: i32,
    record_round: i32,
    record: &PreparedDungeonRecord,
) -> Result<bool> {
    save_prepared_dungeon_record_in_transaction(
        tx,
        user_id,
        episode_id,
        record_round,
        record,
        false,
    )
    .await
}

pub async fn replace_prepared_dungeon_record_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    episode_id: i32,
    record_round: i32,
    record: &PreparedDungeonRecord,
) -> Result<()> {
    save_prepared_dungeon_record_in_transaction(
        tx,
        user_id,
        episode_id,
        record_round,
        record,
        true,
    )
    .await?;
    Ok(())
}

async fn save_prepared_dungeon_record_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    episode_id: i32,
    record_round: i32,
    record: &PreparedDungeonRecord,
    replace: bool,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        INSERT INTO dungeon_records (user_id, episode_id, record_round, hero_list, sub_hero_list, cloth_id, equips, version, created_at, oper_records)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, episode_id) DO UPDATE SET
            record_round = excluded.record_round,
            hero_list = excluded.hero_list,
            sub_hero_list = excluded.sub_hero_list,
            cloth_id = excluded.cloth_id,
            equips = excluded.equips,
            version = excluded.version,
            created_at = excluded.created_at,
            oper_records = excluded.oper_records
        WHERE ? OR excluded.record_round <= dungeon_records.record_round
        "#,
    )
    .bind(user_id)
    .bind(episode_id)
    .bind(record_round)
    .bind(&record.hero_list)
    .bind(&record.sub_hero_list)
    .bind(record.cloth_id)
    .bind(&record.equips)
    .bind(record.version)
    .bind(common::time::ServerTime::now_ms())
    .bind(&record.oper_records)
    .bind(replace)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn load_dungeon_record_operations(
    pool: &SqlitePool,
    user_id: i64,
    episode_id: i32,
) -> Result<Vec<sonettobuf::FightRoundOperRecord>> {
    let value: Option<String> = sqlx::query_scalar(
        "SELECT oper_records FROM dungeon_records WHERE user_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_optional(pool)
    .await?;

    Ok(value
        .map(|value| serde_json::from_str(&value))
        .transpose()?
        .unwrap_or_default())
}

pub async fn dungeon_record_round(
    pool: &SqlitePool,
    user_id: i64,
    episode_id: i32,
) -> Result<Option<i32>> {
    Ok(sqlx::query_scalar(
        "SELECT record_round FROM dungeon_records WHERE user_id = ? AND episode_id = ?",
    )
    .bind(user_id)
    .bind(episode_id)
    .fetch_optional(pool)
    .await?)
}
