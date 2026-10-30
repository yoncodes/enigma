use crate::{
    db::game::{battle, tasks},
    models::game::{tasks::UserTask, weekwalk::*},
};
use anyhow::Result;
use sonettobuf;
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeekwalkEncounterSeed {
    pub battle_id: i32,
    pub element_id: i32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeekwalkMapSeed {
    pub map_id: i32,
    pub scene_id: i32,
    pub layer: i32,
    pub is_deep: bool,
    pub buff_id: i32,
    pub encounters: Vec<WeekwalkEncounterSeed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeekwalkBattleSelection {
    pub map_id: i32,
    pub element_id: i32,
    pub battle_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeekwalkElementState {
    pub current_index: i32,
    pub next_index: i32,
    pub is_finish: bool,
    pub next_map: Option<WeekwalkMapSeed>,
}

pub async fn ensure_cycle(
    pool: &SqlitePool,
    user_id: i64,
    issue_id: i32,
    start_time: i32,
    end_time: i32,
    first_map: &WeekwalkMapSeed,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET updated_at = updated_at WHERE id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let current: Option<(i32, i32)> =
        sqlx::query_as("SELECT issue_id, end_time FROM user_weekwalk_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let has_map: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_weekwalk_maps WHERE user_id = ?)")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
    if current.is_some_and(|(current_issue, current_end)| {
        current_issue == issue_id && current_end > start_time && has_map
    }) {
        let has_first_map: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM user_weekwalk_maps WHERE user_id = ? AND map_id = ?
             )",
        )
        .bind(user_id)
        .bind(first_map.map_id)
        .fetch_one(&mut *tx)
        .await?;
        if !first_map.is_deep && !has_first_map {
            insert_map(&mut tx, user_id, first_map).await?;
        }
        tx.commit().await?;
        return Ok(());
    }

    for table in [
        "user_weekwalk_element_history",
        "user_weekwalk_battle_heroes",
        "user_weekwalk_elements",
        "user_weekwalk_battles",
        "user_weekwalk_heroes",
        "user_weekwalk_stories",
        "user_weekwalk_maps",
        "user_weekwalk_info",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE user_id = ?"))
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO user_weekwalk_info (
            user_id, time, end_time, max_layer, issue_id, is_open_deep, is_pop_deep_rule
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(start_time)
    .bind(end_time)
    .bind(first_map.layer)
    .bind(issue_id)
    .bind(first_map.is_deep)
    .bind(first_map.is_deep)
    .execute(&mut *tx)
    .await?;
    insert_map(&mut tx, user_id, first_map).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn select_battle(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
    element_id: i32,
) -> Result<Option<WeekwalkBattleSelection>> {
    Ok(sqlx::query_as::<_, (i32,)>(
        "SELECT battle.battle_id
         FROM user_weekwalk_battles battle
         JOIN user_weekwalk_maps map
           ON map.user_id = battle.user_id AND map.map_id = battle.map_id
         JOIN user_weekwalk_elements element
           ON element.user_id = battle.user_id
          AND element.map_id = battle.map_id
          AND element.element_id = battle.element_id
         WHERE battle.user_id = ? AND battle.map_id = ? AND battle.element_id = ?
           AND map.is_finished = 0 AND element.is_finish = FALSE AND element.visible = TRUE",
    )
    .bind(user_id)
    .bind(map_id)
    .bind(element_id)
    .fetch_optional(pool)
    .await?
    .map(|(battle_id,)| WeekwalkBattleSelection {
        map_id,
        element_id,
        battle_id,
    }))
}

pub async fn set_hero_group_selection(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
    battle_id: i32,
    selection: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_weekwalk_battles SET hero_group_select = ?
         WHERE user_id = ? AND map_id = ? AND battle_id = ?",
    )
    .bind(selection)
    .bind(user_id)
    .bind(map_id)
    .bind(battle_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn complete_battle(
    pool: &SqlitePool,
    user_id: i64,
    selection: WeekwalkBattleSelection,
    star: i32,
    fight_id: i64,
    reveal_element_id: Option<i32>,
    state: WeekwalkElementState,
) -> Result<Option<Vec<UserTask>>> {
    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE user_weekwalk_battles
         SET star = MAX(star, ?), max_star = MAX(max_star, ?)
         WHERE user_id = ? AND map_id = ? AND battle_id = ? AND element_id = ?
           AND EXISTS (
               SELECT 1 FROM user_weekwalk_maps map
               WHERE map.user_id = user_weekwalk_battles.user_id
                 AND map.map_id = user_weekwalk_battles.map_id
                 AND map.is_finished = 0
           )
           AND EXISTS (
               SELECT 1 FROM user_weekwalk_elements element
               WHERE element.user_id = user_weekwalk_battles.user_id
                 AND element.map_id = user_weekwalk_battles.map_id
                 AND element.element_id = user_weekwalk_battles.element_id
                 AND element.is_finish = FALSE AND element.visible = TRUE
                 AND element.index_num = ?
           )",
    )
    .bind(star)
    .bind(star)
    .bind(user_id)
    .bind(selection.map_id)
    .bind(selection.battle_id)
    .bind(selection.element_id)
    .bind(state.current_index)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed == 0 {
        tx.rollback().await?;
        return Ok(None);
    }
    sqlx::query(
        "UPDATE user_weekwalk_elements SET index_num = ?, is_finish = ?
         WHERE user_id = ? AND map_id = ? AND element_id = ?",
    )
    .bind(state.next_index)
    .bind(state.is_finish)
    .bind(user_id)
    .bind(selection.map_id)
    .bind(selection.element_id)
    .execute(&mut *tx)
    .await?;

    if let Some(element_id) = reveal_element_id {
        sqlx::query(
            "UPDATE user_weekwalk_elements SET visible = TRUE
             WHERE user_id = ? AND map_id = ? AND element_id = ?",
        )
        .bind(user_id)
        .bind(selection.map_id)
        .bind(element_id)
        .execute(&mut *tx)
        .await?;
    }
    finish_map_if_complete(&mut tx, user_id, selection.map_id, state.next_map).await?;
    let task_updates = tasks::sync_weekwalk_tasks_in_transaction(&mut tx, user_id).await?;
    battle::finish_fight_instance_in_transaction(&mut tx, user_id, fight_id).await?;
    tx.commit().await?;
    Ok(Some(task_updates))
}

pub async fn advance_element(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
    element_id: i32,
    state: WeekwalkElementState,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE user_weekwalk_elements SET index_num = ?, is_finish = ?
         WHERE user_id = ? AND map_id = ? AND element_id = ?
           AND index_num = ? AND is_finish = FALSE AND visible = TRUE",
    )
    .bind(state.next_index)
    .bind(state.is_finish)
    .bind(user_id)
    .bind(map_id)
    .bind(element_id)
    .bind(state.current_index)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    finish_map_if_complete(&mut tx, user_id, map_id, state.next_map).await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn replace_element_history(
    pool: &SqlitePool,
    user_id: i64,
    map_id: i32,
    element_id: i32,
    history: &[String],
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM user_weekwalk_elements
            WHERE user_id = ? AND map_id = ? AND element_id = ?
         )",
    )
    .bind(user_id)
    .bind(map_id)
    .bind(element_id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "DELETE FROM user_weekwalk_element_history
         WHERE user_id = ? AND map_id = ? AND element_id = ?",
    )
    .bind(user_id)
    .bind(map_id)
    .bind(element_id)
    .execute(&mut *tx)
    .await?;
    for (order, entry) in history.iter().enumerate() {
        sqlx::query(
            "INSERT INTO user_weekwalk_element_history (
                user_id, map_id, element_id, history_entry, sort_order
             ) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(map_id)
        .bind(element_id)
        .bind(entry)
        .bind(i32::try_from(order)?)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

async fn finish_map_if_complete(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    map_id: i32,
    next_map: Option<WeekwalkMapSeed>,
) -> Result<()> {
    let complete: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS(
            SELECT 1 FROM user_weekwalk_battles battle
            JOIN user_weekwalk_elements element
              ON element.user_id = battle.user_id
             AND element.map_id = battle.map_id
             AND element.element_id = battle.element_id
            WHERE battle.user_id = ? AND battle.map_id = ?
              AND (battle.star = 0 OR element.is_finish = FALSE)
         )",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_one(&mut **tx)
    .await?;
    if complete {
        sqlx::query(
            "UPDATE user_weekwalk_maps SET is_finish = 1, is_finished = 1
             WHERE user_id = ? AND map_id = ?",
        )
        .bind(user_id)
        .bind(map_id)
        .execute(&mut **tx)
        .await?;
        if let Some(next_map) = next_map {
            let has_next_map: bool = sqlx::query_scalar(
                "SELECT EXISTS(
                    SELECT 1 FROM user_weekwalk_maps WHERE user_id = ? AND map_id = ?
                 )",
            )
            .bind(user_id)
            .bind(next_map.map_id)
            .fetch_one(&mut **tx)
            .await?;
            if !has_next_map {
                insert_map(tx, user_id, &next_map).await?;
            }
            sqlx::query(
                "UPDATE user_weekwalk_info
                 SET max_layer = MAX(max_layer, ?),
                     is_open_deep = CASE WHEN ? THEN TRUE ELSE is_open_deep END,
                     is_pop_deep_rule = CASE WHEN ? THEN TRUE ELSE is_pop_deep_rule END,
                     is_pop_shallow_settle = CASE WHEN ? THEN TRUE ELSE is_pop_shallow_settle END
                 WHERE user_id = ?",
            )
            .bind(next_map.layer)
            .bind(next_map.is_deep)
            .bind(next_map.is_deep)
            .bind(next_map.is_deep)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE user_weekwalk_info SET is_pop_deep_settle = TRUE WHERE user_id = ?",
            )
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_map(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    map: &WeekwalkMapSeed,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_weekwalk_maps (
            user_id, map_id, scene_id, buff_id, is_show_buff, is_show_finished
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(map.map_id)
    .bind(map.scene_id)
    .bind(map.buff_id)
    .bind(map.buff_id == 0)
    .bind(map.is_deep)
    .execute(&mut **tx)
    .await?;
    for encounter in &map.encounters {
        sqlx::query(
            "INSERT INTO user_weekwalk_battles (
                user_id, map_id, battle_id, star, max_star, hero_group_select, element_id
             ) VALUES (?, ?, ?, 0, 0, 0, ?)",
        )
        .bind(user_id)
        .bind(map.map_id)
        .bind(encounter.battle_id)
        .bind(encounter.element_id)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO user_weekwalk_elements (
                user_id, map_id, element_id, is_finish, index_num, visible
             ) VALUES (?, ?, ?, FALSE, 0, ?)",
        )
        .bind(user_id)
        .bind(map.map_id)
        .bind(encounter.element_id)
        .bind(encounter.visible)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

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

pub async fn mark_pop_deep_settle(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE user_weekwalk_info
         SET is_pop_deep_settle = FALSE
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
mod tests;
