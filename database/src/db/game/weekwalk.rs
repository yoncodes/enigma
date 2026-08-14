use crate::models::game::weekwalk::*;
use anyhow::Result;
use sonettobuf;
use sqlx::SqlitePool;

pub async fn get_weekwalk_info(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(UserWeekwalkInfo, Vec<sonettobuf::MapInfo>)> {
    // Get main info
    let info =
        sqlx::query_as::<_, UserWeekwalkInfo>("SELECT * FROM user_weekwalk_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(UserWeekwalkInfo {
                user_id,
                time: 0,
                end_time: 0,
                max_layer: 0,
                issue_id: 0,
                is_pop_deep_rule: false,
                is_open_deep: false,
                is_pop_shallow_settle: false,
                is_pop_deep_settle: false,
                deep_progress: String::new(),
                time_this_week: 0,
            });

    // Get all maps
    let maps = sqlx::query_as::<_, WeekwalkMap>(
        "SELECT * FROM user_weekwalk_maps WHERE user_id = ? ORDER BY map_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut map_infos = Vec::new();
    for map in maps {
        // Get battles for this map
        let battles = get_map_battles(pool, user_id, map.map_id).await?;

        // Get elements for this map
        let elements = get_map_elements(pool, user_id, map.map_id).await?;

        // Get heroes for this map
        let heroes = get_map_heroes(pool, user_id, map.map_id).await?;

        // Get story IDs for this map
        let story_ids = get_map_stories(pool, user_id, map.map_id).await?;

        map_infos.push(sonettobuf::MapInfo {
            id: Some(map.map_id),
            scene_id: Some(map.scene_id),
            is_finish: Some(map.is_finish),
            is_finished: Some(map.is_finished),
            buff_id: Some(map.buff_id),
            is_show_buff: Some(map.is_show_buff),
            is_show_finished: Some(map.is_show_finished),
            is_show_select_cd: Some(map.is_show_select_cd),
            battle_infos: battles.into_iter().collect(),
            element_infos: elements.into_iter().map(Into::into).collect(),
            hero_infos: heroes.into_iter().map(Into::into).collect(),
            story_ids,
        });
    }

    Ok((info, map_infos))
}

pub async fn mark_pop_shallow_settle(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE user_weekwalk_info
         SET is_pop_shallow_settle = FALSE
         WHERE user_id = ?",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn get_map_battles(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
) -> Result<Vec<sonettobuf::BattleInfo>> {
    let battles = sqlx::query_as::<_, WeekwalkBattle>(
        "SELECT battle_id, star, max_star, hero_group_select, element_id
         FROM user_weekwalk_battles
         WHERE user_id = ? AND map_id = ?
         ORDER BY battle_id",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_all(pool)
    .await?;

    let mut battle_infos = Vec::new();
    for battle in battles {
        // Get hero IDs for this battle
        let hero_ids = sqlx::query_scalar(
            "SELECT hero_id FROM user_weekwalk_battle_heroes
             WHERE user_id = ? AND map_id = ? AND battle_id = ?",
        )
        .bind(user_id)
        .bind(map_id)
        .bind(battle.battle_id)
        .fetch_all(pool)
        .await?;

        battle_infos.push(sonettobuf::BattleInfo {
            battle_id: Some(battle.battle_id),
            star: Some(battle.star),
            max_star: Some(battle.max_star),
            hero_ids,
            hero_group_select: Some(battle.hero_group_select),
            element_id: Some(battle.element_id),
        });
    }

    Ok(battle_infos)
}

async fn get_map_elements(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
) -> Result<Vec<WeekwalkElementInfo>> {
    let elements: Vec<(i32, bool, i32, bool)> = sqlx::query_as(
        "SELECT element_id, is_finish, index_num, visible
         FROM user_weekwalk_elements
         WHERE user_id = ? AND map_id = ?
         ORDER BY element_id",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_all(pool)
    .await?;

    let mut element_infos = Vec::new();
    for (element_id, is_finish, index_num, visible) in elements {
        // Get history list for this element
        let history_list = sqlx::query_scalar(
            "SELECT history_entry FROM user_weekwalk_element_history
             WHERE user_id = ? AND map_id = ? AND element_id = ?
             ORDER BY sort_order",
        )
        .bind(user_id)
        .bind(map_id)
        .bind(element_id)
        .fetch_all(pool)
        .await?;

        element_infos.push(WeekwalkElementInfo {
            element_id,
            is_finish,
            index_num,
            history_list,
            visible,
        });
    }

    Ok(element_infos)
}

async fn get_map_heroes(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
) -> Result<Vec<WeekwalkHeroInfo>> {
    let heroes: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT hero_id, cd FROM user_weekwalk_heroes
         WHERE user_id = ? AND map_id = ?
         ORDER BY hero_id",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_all(pool)
    .await?;

    Ok(heroes
        .into_iter()
        .map(|(hero_id, cd)| WeekwalkHeroInfo { hero_id, cd })
        .collect())
}

async fn get_map_stories(pool: &SqlitePool, user_id: i64, map_id: i32) -> Result<Vec<i32>> {
    let story_ids = sqlx::query_scalar(
        "SELECT story_id FROM user_weekwalk_stories
         WHERE user_id = ? AND map_id = ?
         ORDER BY story_id",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_all(pool)
    .await?;

    Ok(story_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn shallow_settlement_ack_is_idempotent_and_player_scoped() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();

        for (user_id, username) in [(1, "marked"), (2, "other"), (3, "missing")] {
            sqlx::query(
                "INSERT INTO users (id, username, created_at, updated_at)
                 VALUES (?, ?, 0, 0)",
            )
            .bind(user_id)
            .bind(username)
            .execute(&pool)
            .await
            .unwrap();
        }
        for user_id in [1, 2] {
            sqlx::query(
                "INSERT INTO user_weekwalk_info (user_id, is_pop_shallow_settle)
                 VALUES (?, TRUE)",
            )
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        mark_pop_shallow_settle(&pool, 1).await.unwrap();
        mark_pop_shallow_settle(&pool, 1).await.unwrap();
        mark_pop_shallow_settle(&pool, 3).await.unwrap();

        assert!(
            !get_weekwalk_info(&pool, 1)
                .await
                .unwrap()
                .0
                .is_pop_shallow_settle
        );
        assert!(
            get_weekwalk_info(&pool, 2)
                .await
                .unwrap()
                .0
                .is_pop_shallow_settle
        );
        assert!(
            !get_weekwalk_info(&pool, 3)
                .await
                .unwrap()
                .0
                .is_pop_shallow_settle
        );
        let missing_rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_info WHERE user_id = 3")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(missing_rows, 0);
    }
}
