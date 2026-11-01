use crate::models::game::weekwalk::*;
use anyhow::Result;
use sonettobuf;
use sqlx::SqlitePool;

/// The client's first map tutorial explicitly points to 10111 and then emits
/// GuideShowElement10112. Implement only this confirmed opening transition and
/// 10112's configured dialog -> battle transition; other branches need data.
pub async fn advance_opening_dialog(
    pool: &SqlitePool, user_id: i64, map_id: i32, element_id: i32, option: i32,
) -> Result<bool> {
    if map_id != 101 { return Ok(false); }
    let mut tx = pool.begin().await?;
    let matches: Vec<(bool, i32, bool)> = sqlx::query_as(
        "SELECT is_finish, index_num, visible FROM user_weekwalk_elements WHERE user_id = ? AND map_id = ? AND element_id = ?",
    ).bind(user_id).bind(map_id).bind(element_id).fetch_all(&mut *tx).await?;
    let [(finished, index, visible)] = matches.as_slice() else { return Ok(false); };
    if !visible { return Ok(false); }
    match (element_id, *finished, *index, option) {
        (10111, true, 0, 1) => {
            let next: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_elements WHERE user_id = ? AND map_id = 101 AND element_id = 10112")
                .bind(user_id).fetch_one(&mut *tx).await?;
            if next != 1 { return Ok(false); }
        }
        (10111, false, 0, 1) => {
            let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_elements WHERE user_id = ? AND map_id = 101 AND element_id = 10112")
                .bind(user_id).fetch_one(&mut *tx).await?;
            if existing != 0 { return Ok(false); }
            // The five encounters and their element IDs are explicit in the
            // matching client's weekwalk_element.json. Never replace old rows.
            let battles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_battles WHERE user_id = ? AND map_id = 101")
                .bind(user_id).fetch_one(&mut *tx).await?;
            if battles != 0 { return Ok(false); }
            for (battle_id, next_element) in [(800101,10112), (800102,10113), (800103,10115), (800104,10114), (800105,10117)] {
                sqlx::query("INSERT INTO user_weekwalk_battles (user_id, map_id, battle_id, element_id) VALUES (?, 101, ?, ?)")
                    .bind(user_id).bind(battle_id).bind(next_element).execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE user_weekwalk_elements SET is_finish = TRUE WHERE user_id = ? AND map_id = 101 AND element_id = 10111")
                .bind(user_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO user_weekwalk_elements (user_id, map_id, element_id, visible) VALUES (?, 101, 10112, TRUE)")
                .bind(user_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO user_weekwalk_stories (user_id, map_id, story_id) VALUES (?, 101, 101111) ON CONFLICT DO NOTHING")
                .bind(user_id).execute(&mut *tx).await?;
        }
        // The client sends its default option 0 for this dialog. Its next
        // type/parameter are Battle and 800101, respectively.
        (10112, false, 0, 0) => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_battles WHERE user_id = ? AND map_id = 101 AND element_id = 10112 AND battle_id = 800101")
                .bind(user_id).fetch_one(&mut *tx).await?;
            if count != 1 { return Ok(false); }
            sqlx::query("UPDATE user_weekwalk_elements SET index_num = 1 WHERE user_id = ? AND map_id = 101 AND element_id = 10112")
                .bind(user_id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM user_weekwalk_element_history WHERE user_id = ? AND map_id = 101 AND element_id = 10112")
                .bind(user_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO user_weekwalk_stories (user_id, map_id, story_id) VALUES (?, 101, 101121) ON CONFLICT DO NOTHING")
                .bind(user_id).execute(&mut *tx).await?;
        }
        (10112, false, 1, 0) => {} // repeated request, already awaiting battle
        _ => return Ok(false),
    }
    tx.commit().await?;
    Ok(true)
}

pub async fn opening_battle_available(pool: &SqlitePool, user_id: i64, map_id: i32, element_id: i32) -> Result<bool> {
    if (map_id, element_id) != (101, 10112) { return Ok(false); }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_elements e \
        JOIN user_weekwalk_battles b ON e.user_id = b.user_id AND e.map_id = b.map_id AND e.element_id = b.element_id \
        WHERE e.user_id = ? AND e.map_id = 101 AND e.element_id = 10112 AND e.visible = 1 AND e.is_finish = 0 \
          AND e.index_num = 1 AND b.battle_id = 800101")
        .bind(user_id).fetch_one(pool).await?;
    Ok(count == 1)
}

pub async fn settle_opening_battle(pool: &SqlitePool, user_id: i64, fight_id: i64, battle_id: i32, star: Option<i32>) -> Result<bool> {
    if battle_id != 800101 || star.is_some_and(|s| !(1..=2).contains(&s)) { return Ok(false); }
    let mut tx = pool.begin().await?;
    let matching: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE id = ? AND user_id = ? AND episode_id = 90101 AND battle_id = 800101 AND active = 1")
        .bind(fight_id).bind(user_id).fetch_one(&mut *tx).await?;
    let nodes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_elements e JOIN user_weekwalk_battles b \
        ON e.user_id = b.user_id AND e.map_id = b.map_id AND e.element_id = b.element_id \
        WHERE e.user_id = ? AND e.map_id = 101 AND e.element_id = 10112 AND e.index_num = 1 \
          AND e.visible = 1 AND e.is_finish = 0 AND b.battle_id = 800101")
        .bind(user_id).fetch_one(&mut *tx).await?;
    if matching != 1 || nodes != 1 { return Ok(false); }
    if let Some(star) = star {
        sqlx::query("UPDATE user_weekwalk_battles SET star = ?, max_star = MAX(max_star, ?) WHERE user_id = ? AND map_id = 101 AND battle_id = 800101 AND element_id = 10112")
            .bind(star).bind(star).bind(user_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE user_weekwalk_elements SET index_num = 2 WHERE user_id = ? AND map_id = 101 AND element_id = 10112")
            .bind(user_id).execute(&mut *tx).await?;
    }
    crate::db::game::battle::finish_fight_instance_in_transaction(&mut tx, user_id, fight_id).await?;
    tx.commit().await?;
    Ok(true)
}

/// Create only the first, explicitly configured shallow-dream entrance for a
/// completely new profile. Do not replace any existing or partial progress.
pub async fn ensure_initial_shallow_map(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO user_weekwalk_info (user_id, max_layer)
         SELECT ?, 1
         WHERE EXISTS (SELECT 1 FROM user_open_infos WHERE user_id = ? AND open_id = 36 AND is_open = 1)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_info WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_maps WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_elements WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_battles WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_heroes WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_stories WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_element_history WHERE user_id = ?)
           AND NOT EXISTS (SELECT 1 FROM user_weekwalk_battle_heroes WHERE user_id = ?)",
    )
    .bind(user_id).bind(user_id).bind(user_id).bind(user_id).bind(user_id)
    .bind(user_id).bind(user_id).bind(user_id).bind(user_id).bind(user_id)
    .execute(&mut *tx).await?;
    if inserted.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(false);
    }

    // weekwalk.json: 101 is the first layer, with scene 101. The client's
    // weekwalk_branch.json identifies 10111 as the shared opening node.
    sqlx::query("INSERT INTO user_weekwalk_maps (user_id, map_id, scene_id) VALUES (?, 101, 101)")
        .bind(user_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO user_weekwalk_elements (user_id, map_id, element_id, visible) VALUES (?, 101, 10111, TRUE)")
        .bind(user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

/// The client sends its dialog position as section#index pairs. Replace the
/// history atomically, without changing any node, battle or reward state.
pub async fn save_dialog_history(
    pool: &SqlitePool, user_id: i64, map_id: i32, element_id: i32, history: &[String],
) -> Result<bool> {
    if history.len() > 128 || history.iter().any(|entry| {
        if entry.len() > 32 { return true; }
        match entry.split_once('#') {
            Some((section, index)) => section.is_empty() || index.is_empty()
                || !section.bytes().all(|b| b.is_ascii_digit())
                || !index.bytes().all(|b| b.is_ascii_digit()),
            None => true,
        }
    }) {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_weekwalk_elements WHERE user_id = ? AND map_id = ? AND element_id = ?",
    ).bind(user_id).bind(map_id).bind(element_id).fetch_one(&mut *tx).await?;
    if count != 1 { return Ok(false); }
    sqlx::query("DELETE FROM user_weekwalk_element_history WHERE user_id = ? AND map_id = ? AND element_id = ?")
        .bind(user_id).bind(map_id).bind(element_id).execute(&mut *tx).await?;
    for (order, entry) in history.iter().enumerate() {
        sqlx::query("INSERT INTO user_weekwalk_element_history (user_id, map_id, element_id, history_entry, sort_order) VALUES (?, ?, ?, ?, ?)")
            .bind(user_id).bind(map_id).bind(element_id).bind(entry).bind(order as i32)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(true)
}

pub async fn set_battle_team(pool: &SqlitePool, user_id: i64, map_id: i32, battle_id: i32, select: i32) -> Result<bool> {
    if select < 0 { return Ok(false); }
    let result = sqlx::query("UPDATE user_weekwalk_battles SET hero_group_select = ? WHERE user_id = ? AND map_id = ? AND battle_id = ? \
        AND 1 = (SELECT COUNT(*) FROM user_weekwalk_battles WHERE user_id = ? AND map_id = ? AND battle_id = ?)")
        .bind(select).bind(user_id).bind(map_id).bind(battle_id)
        .bind(user_id).bind(map_id).bind(battle_id).execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn clear_map_notice(pool: &SqlitePool, user_id: i64, map_id: i32, buff: bool) -> Result<bool> {
    let statement = if buff {
        "UPDATE user_weekwalk_maps SET is_show_buff = FALSE WHERE user_id = ? AND map_id = ?"
    } else {
        "UPDATE user_weekwalk_maps SET is_show_finished = FALSE WHERE user_id = ? AND map_id = ?"
    };
    Ok(sqlx::query(statement).bind(user_id).bind(map_id).execute(pool).await?.rows_affected() == 1)
}

pub async fn clear_deep_rule_notice(pool: &SqlitePool, user_id: i64) -> Result<()> {
    sqlx::query("UPDATE user_weekwalk_info SET is_pop_deep_rule = FALSE WHERE user_id = ?")
        .bind(user_id).execute(pool).await?;
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
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn isolated_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        for id in 1..=5 {
            sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
                .bind(id).bind(format!("weekwalk-test-{id}")).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn opening_dialog_reveals_configured_first_battle_without_resetting_existing_progress() {
        let pool = isolated_pool().await;
        for user in [1, 2] {
            sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 36, 1)")
                .bind(user).execute(&pool).await.unwrap();
            ensure_initial_shallow_map(&pool, user).await.unwrap();
        }
        assert!(save_dialog_history(&pool, 1, 101, 10111, &["0#9".into()]).await.unwrap());
        assert!(!advance_opening_dialog(&pool, 1, 101, 10111, 2).await.unwrap());
        assert!(advance_opening_dialog(&pool, 1, 101, 10111, 1).await.unwrap());
        assert!(advance_opening_dialog(&pool, 1, 101, 10111, 1).await.unwrap());
        let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
        assert_eq!(maps[0].battle_infos.len(), 5);
        assert_eq!(maps[0].battle_infos[0].battle_id, Some(800101));
        assert_eq!(maps[0].element_infos[0].is_finish, Some(true));
        assert_eq!(maps[0].element_infos[1].element_id, Some(10112));
        assert_eq!(maps[0].element_infos[1].index, Some(0));
        assert_eq!(maps[0].element_infos[0].historylist, ["0#9"]);
        assert!(!opening_battle_available(&pool, 1, 101, 10112).await.unwrap());
        assert!(!advance_opening_dialog(&pool, 2, 101, 10112, 0).await.unwrap());
        assert!(advance_opening_dialog(&pool, 1, 101, 10112, 0).await.unwrap());
        assert!(advance_opening_dialog(&pool, 1, 101, 10112, 0).await.unwrap());
        assert!(opening_battle_available(&pool, 1, 101, 10112).await.unwrap());
        assert!(!opening_battle_available(&pool, 2, 101, 10112).await.unwrap());
        assert_eq!(get_weekwalk_info(&pool, 2).await.unwrap().1[0].element_infos.len(), 1);
    }

    #[tokio::test]
    async fn opening_battle_only_advances_on_victory_and_cannot_settle_twice() {
        let pool = isolated_pool().await;
        sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (1, 36, 1)")
            .execute(&pool).await.unwrap();
        ensure_initial_shallow_map(&pool, 1).await.unwrap();
        advance_opening_dialog(&pool, 1, 101, 10111, 1).await.unwrap();
        advance_opening_dialog(&pool, 1, 101, 10112, 0).await.unwrap();
        async fn create(pool: &SqlitePool) -> i64 {
            crate::db::game::battle::create_fight_instance(pool, crate::db::game::battle::NewFightInstance {
                user_id: 1, episode_id: 90101, battle_id: 800101, multiplication: 1,
                entry_cost: "{}", checkpoint: "{}", created_at: 0,
            }).await.unwrap()
        }
        let first = create(&pool).await;
        assert!(!settle_opening_battle(&pool, 2, first, 800101, Some(2)).await.unwrap());
        assert!(!settle_opening_battle(&pool, 1, first, 800102, Some(2)).await.unwrap());
        assert!(settle_opening_battle(&pool, 1, first, 800101, None).await.unwrap());
        assert!(!settle_opening_battle(&pool, 1, first, 800101, Some(2)).await.unwrap());
        assert!(opening_battle_available(&pool, 1, 101, 10112).await.unwrap());
        let second = create(&pool).await;
        assert!(settle_opening_battle(&pool, 1, second, 800101, Some(2)).await.unwrap());
        let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
        assert_eq!((maps[0].battle_infos[0].star, maps[0].battle_infos[0].max_star), (Some(2), Some(2)));
        assert_eq!(maps[0].element_infos[1].index, Some(2));
        assert!(!opening_battle_available(&pool, 1, 101, 10112).await.unwrap());
        assert!(!settle_opening_battle(&pool, 1, second, 800101, Some(2)).await.unwrap());
        let cleared: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = 1 AND episode_id = 90101")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(cleared, 0, "one fight must not clear the whole mode");
    }

    #[tokio::test]
    async fn initial_map_requires_unlock_and_never_overwrites_existing_progress() {
        let pool = isolated_pool().await;
        assert!(!ensure_initial_shallow_map(&pool, 1).await.unwrap());
        for user in 1..=4 {
            sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 36, TRUE)")
                .bind(user).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO user_weekwalk_info (user_id, max_layer, issue_id) VALUES (2, 7, 4)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_weekwalk_maps (user_id, map_id, scene_id) VALUES (3, 102, 102)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_weekwalk_element_history VALUES (4, 101, 10111, '0#3', 0)")
            .execute(&pool).await.unwrap();
        assert!(ensure_initial_shallow_map(&pool, 1).await.unwrap());
        assert!(!ensure_initial_shallow_map(&pool, 1).await.unwrap());
        for user in 2..=4 { assert!(!ensure_initial_shallow_map(&pool, user).await.unwrap()); }
        let (profile, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
        assert_eq!(profile.max_layer, 1);
        assert_eq!(maps.len(), 1);
        assert_eq!((maps[0].id, maps[0].scene_id), (Some(101), Some(101)));
        assert_eq!(maps[0].element_infos.len(), 1);
        assert_eq!((maps[0].element_infos[0].element_id, maps[0].element_infos[0].visible), (Some(10111), Some(true)));
        assert!(maps[0].battle_infos.is_empty(), "do not invent battles before their unlock rules are known");
        assert_eq!((get_weekwalk_info(&pool, 2).await.unwrap().0.max_layer, get_weekwalk_info(&pool, 2).await.unwrap().0.issue_id), (7, 4));
        assert_eq!(get_weekwalk_info(&pool, 3).await.unwrap().1[0].id, Some(102));
    }

    #[tokio::test]
    async fn dialogue_history_is_saved_atomically_and_player_scoped() {
        let pool = isolated_pool().await;
        for user in [1, 2] {
            sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 36, 1)")
                .bind(user).execute(&pool).await.unwrap();
            ensure_initial_shallow_map(&pool, user).await.unwrap();
        }
        assert!(save_dialog_history(&pool, 1, 101, 10111, &["0#3".into(), "1#7".into()]).await.unwrap());
        assert!(!save_dialog_history(&pool, 1, 101, 10111, &["bad entry".into()]).await.unwrap());
        assert!(!save_dialog_history(&pool, 1, 101, 10112, &["0#5".into()]).await.unwrap());
        let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
        assert_eq!(maps[0].element_infos[0].historylist, ["0#3", "1#7"]);
        assert_eq!(maps[0].element_infos[0].is_finish, Some(false));
        assert!(get_weekwalk_info(&pool, 2).await.unwrap().1[0].element_infos[0].historylist.is_empty());
        assert!(save_dialog_history(&pool, 1, 101, 10111, &["0#4".into()]).await.unwrap());
        assert_eq!(get_weekwalk_info(&pool, 1).await.unwrap().1[0].element_infos[0].historylist, ["0#4"]);
    }

    #[tokio::test]
    async fn team_selection_and_notices_do_not_modify_other_players_or_ambiguous_battles() {
        let pool = isolated_pool().await;
        for user in [1, 2] {
            sqlx::query("INSERT INTO user_weekwalk_maps (user_id, map_id, scene_id, is_show_buff, is_show_finished) VALUES (?, 101, 101, 1, 1)")
                .bind(user).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO user_weekwalk_battles (user_id, map_id, battle_id) VALUES (?, 101, 800101)")
                .bind(user).execute(&pool).await.unwrap();
        }
        assert!(set_battle_team(&pool, 1, 101, 800101, 3).await.unwrap());
        assert!(!set_battle_team(&pool, 1, 101, 800101, -1).await.unwrap());
        assert!(clear_map_notice(&pool, 1, 101, true).await.unwrap());
        let maps1 = get_weekwalk_info(&pool, 1).await.unwrap().1;
        let maps2 = get_weekwalk_info(&pool, 2).await.unwrap().1;
        assert_eq!(maps1[0].battle_infos[0].hero_group_select, Some(3));
        assert_eq!(maps2[0].battle_infos[0].hero_group_select, Some(0));
        assert_eq!((maps1[0].is_show_buff, maps1[0].is_show_finished), (Some(false), Some(true)));
        assert_eq!(maps2[0].is_show_buff, Some(true));
        sqlx::query("INSERT INTO user_weekwalk_battles (user_id, map_id, battle_id) VALUES (1, 101, 800101)").execute(&pool).await.unwrap();
        assert!(!set_battle_team(&pool, 1, 101, 800101, 4).await.unwrap());
        let selected: Vec<i32> = sqlx::query_scalar("SELECT hero_group_select FROM user_weekwalk_battles WHERE user_id = 1 ORDER BY id")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(selected, vec![3, 0]);
    }

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
