//! Map entry and resume primitives. They are not yet wired to the network router.

use anyhow::{Context, Result, ensure};
use config::explore_layout::Layout;
use database::db::game::explore_runtime::{self, MapState};
use sonettobuf::{ChangeMapReply, GetExploreInfoReply};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::explore_engine;

pub(crate) fn map_totals(layout: &Layout) -> Result<(i32, i32, i32)> {
    let mut chest_ids = std::collections::BTreeSet::new();
    let mut gold = 0;
    let mut purple = 0;
    for unit in layout.areas.iter().flat_map(|area| &area.units) {
        if unit.kind == 1 {
            for effect in &unit.trigger_effects {
                if effect.0 == 15 {
                    let id = effect.1.as_i64().and_then(|v| i32::try_from(v).ok())
                        .or_else(|| effect.1.as_str().and_then(|v| v.parse().ok()))
                        .context("invalid counted chest reference")?;
                    let chest = config::explore_rewards::chest(id)?.context("unknown counted chest")?;
                    if chest.is_count == 1 {
                        ensure!((chest.chapter_id, chest.episode_id) == (layout.chapter_id, layout.episode_id),
                            "counted chest belongs to another map");
                        ensure!(chest_ids.insert(id), "duplicate counted chest in map");
                    }
                }
            }
        } else if unit.kind == 221 {
            match unit.special_data.first().and_then(|value| value.as_str()) {
                Some("1") => gold += 1,
                Some("2") => purple += 1,
                _ => anyhow::bail!("unknown exploration coin type"),
            }
        }
    }
    Ok((i32::try_from(chest_ids.len())?, gold, purple))
}

pub(crate) async fn ensure_map_summary(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, layout: &Layout,
) -> Result<()> {
    let (bonuses, gold, purple) = map_totals(layout)?;
    sqlx::query("INSERT INTO user_explore_maps (user_id, map_id, bonus_num_total, gold_coin_total, purple_coin_total) \
        VALUES (?, ?, ?, ?, ?) ON CONFLICT(user_id, map_id) DO NOTHING")
        .bind(user_id).bind(layout.map_id).bind(bonuses).bind(gold).bind(purple)
        .execute(&mut **tx).await?;
    Ok(())
}

pub async fn change_map(pool: &SqlitePool, user_id: i64, map_id: i32) -> Result<ChangeMapReply> {
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let mut tx = pool.begin().await?;
    let opened: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_open_infos WHERE user_id = ? AND open_id = 47 AND is_open = 1)",
    ).bind(user_id).fetch_one(&mut *tx).await?;
    ensure!(opened, "exploration feature is not unlocked");
    let profile_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_explore_runtime_profiles WHERE user_id = ?)",
    ).bind(user_id).fetch_one(&mut *tx).await?;
    explore_runtime::ensure_profile(&mut tx, user_id).await?;
    if let Some(previous_map_id) = explore_runtime::current_map(&mut tx, user_id).await? {
        if previous_map_id != map_id {
            let previous_layout = config::explore_layout::layout(previous_map_id)?.context("unknown previous exploration map")?;
            let previous = explore_runtime::load_map(&mut tx, user_id, previous_map_id).await?.context("missing previous map state")?;
            let selected = explore_runtime::use_item_uid(&mut tx, user_id).await?.context("missing exploration profile")?;
            ensure!(crate::explore_pipe::carried_pot(&previous.state, previous_layout, selected)?.is_none(),
                "put down the carried pipe pot before changing maps");
        }
    }
    if !profile_exists {
        // A new exploration profile starts with map 101, never with arbitrary maps.
        sqlx::query("INSERT INTO user_explore_unlocked_maps (user_id, map_id) VALUES (?, 101)")
            .bind(user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_explore_chapters (user_id, chapter_id) VALUES (?, 1401) \
            ON CONFLICT(user_id, chapter_id) DO NOTHING")
            .bind(user_id).execute(&mut *tx).await?;
    }
    let unlocked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_explore_unlocked_maps WHERE user_id = ? AND map_id = ?)",
    ).bind(user_id).bind(map_id).fetch_one(&mut *tx).await?;
    ensure!(unlocked, "exploration map is not unlocked");
    let pending_fight: Option<Option<i64>> = sqlx::query_scalar(
        "SELECT fight_id FROM user_explore_pending_battles WHERE user_id = ?",
    ).bind(user_id).fetch_optional(&mut *tx).await?;
    ensure!(!matches!(pending_fight, Some(Some(_))), "an exploration fight is still active");
    if pending_fight.is_some() {
        sqlx::query("DELETE FROM user_explore_pending_battles WHERE user_id = ? AND fight_id IS NULL")
            .bind(user_id).execute(&mut *tx).await?;
    }
    let first_enter = explore_runtime::load_map(&mut tx, user_id, map_id).await?.is_none();
    if first_enter {
        let mut initial = MapState::new(layout);
        crate::explore_rune::initialize_counter_doors(&mut initial, layout)?;
        if map_id == 301 {
            crate::explore_light::sync_light(&mut initial, layout, &mut Default::default())?;
        }
        let inserted = explore_runtime::insert_map(&mut tx, user_id, &initial).await?;
        ensure!(inserted, "exploration map was created concurrently");
    }
    ensure_map_summary(&mut tx, user_id, layout).await?;
    explore_runtime::set_current_map(&mut tx, user_id, map_id).await?;
    sqlx::query("INSERT INTO user_explore_info (user_id, last_map_id) VALUES (?, ?) \
        ON CONFLICT(user_id) DO UPDATE SET last_map_id = excluded.last_map_id")
        .bind(user_id).bind(map_id).execute(&mut *tx).await?;
    let stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    let items = explore_runtime::list_items(&mut tx, user_id).await?;
    let use_item_uid = explore_runtime::use_item_uid(&mut tx, user_id).await?
        .context("exploration profile is missing")?;
    let info = explore_engine::map_info(&stored.state, first_enter, items, use_item_uid);
    tx.commit().await?;
    Ok(ChangeMapReply { map_id: Some(map_id), explore_info: Some(info) })
}

pub async fn current_info(pool: &SqlitePool, user_id: i64) -> Result<GetExploreInfoReply> {
    let mut tx = pool.begin().await?;
    let pending_fight: Option<Option<i64>> = sqlx::query_scalar(
        "SELECT fight_id FROM user_explore_pending_battles WHERE user_id = ?",
    ).bind(user_id).fetch_optional(&mut *tx).await?;
    ensure!(!matches!(pending_fight, Some(Some(_))), "an exploration fight is still active");
    if pending_fight.is_some() {
        sqlx::query("DELETE FROM user_explore_pending_battles WHERE user_id = ? AND fight_id IS NULL")
            .bind(user_id).execute(&mut *tx).await?;
    }
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    let items = explore_runtime::list_items(&mut tx, user_id).await?;
    let use_item_uid = explore_runtime::use_item_uid(&mut tx, user_id).await?
        .context("exploration profile is missing")?;
    let info = explore_engine::map_info(&stored.state, false, items, use_item_uid);
    tx.commit().await?;
    Ok(GetExploreInfoReply { explore_info: Some(info) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_ice_subareas_are_open_but_reveal_areas_remain_closed() {
        let first = MapState::new(config::explore_layout::layout(101).unwrap().unwrap());
        assert!(first.areas.contains(&1));
        assert!(!first.areas.contains(&1001));
        let layout = config::explore_layout::layout(102).unwrap().unwrap();
        let mut second = MapState::new(layout);
        assert_eq!(second.areas, [1000, 2001, 2002, 2003, 2004].into_iter().collect());
        second.x = 15;
        second.y = 14;
        explore_engine::move_basic(&mut second, layout, 16, 14).unwrap();
        assert_eq!((second.x, second.y), (16, 14));
        let third = MapState::new(config::explore_layout::layout(103).unwrap().unwrap());
        assert!([1, 2001, 2002, 2003, 2004].iter().all(|area| third.areas.contains(area)));
        let light = MapState::new(config::explore_layout::layout(301).unwrap().unwrap());
        assert!(light.areas.contains(&1000) && light.areas.contains(&5001));
        assert!([1001, 1002, 1003].iter().all(|area| !light.areas.contains(area)));
    }

    #[test]
    fn light_chapter_can_leave_its_initial_marker() {
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        explore_engine::move_basic(&mut map, layout, -6, -5).unwrap();
        assert_eq!((map.x, map.y), (-6, -5));
        assert_eq!(map.objects[&396].status & 2, 0);
    }

    async fn setup() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (920001, 'isolated-entry-test', 0, 0)")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn entry_requires_unlock_and_resume_restores_the_same_map() {
        let pool = setup().await;
        assert!(change_map(&pool, 920001, 101).await.is_err());
        sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (920001, 47, 1)")
            .execute(&pool).await.unwrap();
        assert!(change_map(&pool, 920001, 102).await.is_err());
        let reply = change_map(&pool, 920001, 101).await.unwrap();
        let map = reply.explore_info.unwrap().explore_map.unwrap();
        assert_eq!((map.map_id, map.posx, map.posy, map.is_first_enter), (Some(101), Some(1), Some(6), Some(true)));
        assert_eq!(map.interacts.len(), 199);
        let totals: (i32, i32, i32) = sqlx::query_as("SELECT bonus_num_total, gold_coin_total, purple_coin_total \
            FROM user_explore_maps WHERE user_id = 920001 AND map_id = 101")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(totals, (2, 2, 25));

        let mut tx = pool.begin().await.unwrap();
        let mut stored = explore_runtime::load_map(&mut tx, 920001, 101).await.unwrap().unwrap();
        explore_engine::move_basic(&mut stored.state, config::explore_layout::layout(101).unwrap().unwrap(), 1, 5).unwrap();
        explore_runtime::save_map(&mut tx, 920001, &mut stored).await.unwrap();
        explore_runtime::grant_item(&mut tx, 920001, 150002, 1).await.unwrap();
        tx.commit().await.unwrap();

        let resume = current_info(&pool, 920001).await.unwrap().explore_info.unwrap();
        assert_eq!((resume.explore_map.as_ref().unwrap().posx, resume.explore_map.as_ref().unwrap().posy), (Some(1), Some(5)));
        assert_eq!(resume.explore_items.len(), 1);
        let reopen = change_map(&pool, 920001, 101).await.unwrap().explore_info.unwrap().explore_map.unwrap();
        assert_eq!((reopen.posx, reopen.posy, reopen.is_first_enter), (Some(1), Some(5), Some(false)));
    }
}
