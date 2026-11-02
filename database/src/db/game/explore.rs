use crate::models::game::explore::*;
use anyhow::Result;
use sqlx::SqlitePool;

pub async fn get_explore_info(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(
    UserExploreInfo,
    Vec<ExploreChapterSimple>,
    Vec<ExploreMapSimple>,
    Vec<i32>,
)> {
    // Get main info
    let info =
        sqlx::query_as::<_, UserExploreInfo>("SELECT * FROM user_explore_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(UserExploreInfo {
                user_id,
                last_map_id: 0,
                is_show_bag: false,
            });

    // Get chapters
    let chapters = get_explore_chapters(pool, user_id).await?;

    // Get maps
    let maps = get_explore_maps(pool, user_id).await?;

    // Get unlocked map IDs
    let unlocked_maps = sqlx::query_scalar(
        "SELECT map_id FROM user_explore_unlocked_maps WHERE user_id = ? ORDER BY map_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok((info, chapters, maps, unlocked_maps))
}

async fn get_explore_chapters(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<ExploreChapterSimple>> {
    let chapters = sqlx::query_as::<_, ExploreChapter>(
        "SELECT chapter_id, is_finish FROM user_explore_chapters WHERE user_id = ? ORDER BY chapter_id"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for chapter in chapters {
        // Get archive IDs
        let archive_ids = sqlx::query_scalar(
            "SELECT archive_id FROM user_explore_chapter_archives WHERE user_id = ? AND chapter_id = ? ORDER BY archive_id"
        )
        .bind(user_id)
        .bind(chapter.chapter_id)
        .fetch_all(pool)
        .await?;

        // Get bonus scenes
        let bonus_scene_ids: Vec<i32> = sqlx::query_scalar(
            "SELECT bonus_scene_id FROM user_explore_bonus_scenes WHERE user_id = ? AND chapter_id = ? ORDER BY bonus_scene_id"
        )
        .bind(user_id)
        .bind(chapter.chapter_id)
        .fetch_all(pool)
        .await?;

        let mut bonus_scenes = Vec::new();
        for bonus_scene_id in bonus_scene_ids {
            let ordered: Option<String> = sqlx::query_scalar(
                "SELECT options_json FROM user_explore_runtime_bonus_options WHERE user_id = ? AND chapter_id = ? AND bonus_scene_id = ?"
            ).bind(user_id).bind(chapter.chapter_id).bind(bonus_scene_id).fetch_optional(pool).await?;
            let options = if let Some(json) = ordered {
                serde_json::from_str::<Vec<i32>>(&json)?
            } else {
                sqlx::query_scalar(
                    "SELECT option_id FROM user_explore_bonus_scene_options WHERE user_id = ? AND chapter_id = ? AND bonus_scene_id = ? ORDER BY option_id"
                ).bind(user_id).bind(chapter.chapter_id).bind(bonus_scene_id).fetch_all(pool).await?
            };

            bonus_scenes.push(BonusSceneInfo {
                bonus_scene_id,
                options,
            });
        }

        result.push(ExploreChapterSimple {
            chapter_id: chapter.chapter_id,
            archive_ids,
            bonus_scenes,
            is_finish: chapter.is_finish,
        });
    }

    Ok(result)
}

async fn get_explore_maps(pool: &SqlitePool, user_id: i64) -> Result<Vec<ExploreMapSimple>> {
    let maps = sqlx::query_as::<_, ExploreMap>(
        "SELECT map_id, bonus_num, gold_coin, purple_coin, bonus_num_total, gold_coin_total, purple_coin_total
         FROM user_explore_maps WHERE user_id = ? ORDER BY map_id"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for map in maps {
        let bonus_ids = sqlx::query_scalar(
            "SELECT bonus_id FROM user_explore_map_bonuses WHERE user_id = ? AND map_id = ? ORDER BY bonus_id"
        )
        .bind(user_id)
        .bind(map.map_id)
        .fetch_all(pool)
        .await?;

        result.push(ExploreMapSimple { map, bonus_ids });
    }

    Ok(result)
}
