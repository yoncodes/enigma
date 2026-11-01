//! Return from an ice puzzle to the last save-point at its boundary.
//!
//! The uploaded 102/103 layouts place a save-point at every boundary of a
//! resettable ice area. Collected coins and other progress are never reset.

use std::collections::BTreeSet;
use anyhow::{Context, Result, ensure};
use database::db::game::explore_runtime;
use sonettobuf::ExploreStep;
use sqlx::SqlitePool;

use crate::explore_engine::{changed_object_steps, position_step, sync_gravity_plates};

pub async fn reset(pool: &SqlitePool, user_id: i64) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    let current = layout.node(stored.state.x, stored.state.y).context("player is outside the map")?;
    let area = layout.areas.iter().find(|area| area.id == current.3).context("unknown map area")?;
    ensure!(area.resettable, "current map area cannot be reset");
    // This behavior is restricted to the known ice areas, with no puzzle
    // objects to reset or paid rewards to regenerate.
    ensure!(area.units.iter().all(|u| matches!(u.kind, 206 | 207 | 221)), "unsupported resettable area");
    let (x, y, direction) = stored.state.reset_anchor.context("no saved entrance for this ice area")?;
    ensure!(matches!(direction, 0 | 90 | 180 | 270), "saved entrance direction is invalid");
    let anchor_node = layout.node(x, y).context("saved entrance is not on the map")?;
    ensure!(anchor_node.3 == 0 || stored.state.areas.contains(&anchor_node.3), "saved entrance is in a closed area");
    ensure!(layout.areas.iter().flat_map(|a| &a.units).any(|u| u.kind == 231 && (u.x, u.y) == (x, y)
        && stored.state.objects.get(&u.id).is_some_and(|o| o.status & 3 == 3)), "saved entrance is unavailable");
    ensure!(layout.nodes.iter().any(|node| node.3 == area.id && node.2 == anchor_node.2
        && (i64::from(node.0) - i64::from(x)).abs() + (i64::from(node.1) - i64::from(y)).abs() == 1),
        "saved entrance belongs to another area");
    ensure!(!stored.state.objects.iter().any(|(&id, o)| layout.unit(id).is_none()
        && o.status & 1 != 0 && (o.x, o.y) == (x, y)), "saved entrance is occupied");
    stored.state.x = x;
    stored.state.y = y;
    stored.state.direction = direction;
    let mut changed = BTreeSet::new();
    sync_gravity_plates(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    steps.push(position_step(&stored.state));
    tx.commit().await?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn reset_returns_to_recorded_ice_entrance_without_losing_progress() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 994001;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-reset-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(102).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = 14; map.y = 14;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
        tx.commit().await.unwrap();
        assert!(reset(&pool, user_id).await.is_err());
        crate::explore_move::move_player(&pool, user_id, 15, 14).await.unwrap();
        crate::explore_move::move_player(&pool, user_id, 16, 14).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let mut stored = explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap();
        let coin = layout.areas.iter().flat_map(|a| &a.units).find(|u| u.kind == 221).unwrap().id;
        stored.state.objects.get_mut(&coin).unwrap().status &= !1;
        explore_runtime::save_map(&mut tx, user_id, &mut stored).await.unwrap();
        tx.commit().await.unwrap();
        let steps = reset(&pool, user_id).await.unwrap();
        let move_step: serde_json::Value = serde_json::from_str(steps.last().unwrap().param.as_ref().unwrap()).unwrap();
        assert_eq!((move_step["stepType"].as_i64(), move_step["x"].as_i64(), move_step["y"].as_i64()),
            (Some(1), Some(15), Some(14)));
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap().state;
        assert_eq!((map.x, map.y), (15, 14));
        assert_eq!(map.objects[&coin].status & 1, 0);
        tx.commit().await.unwrap();
        assert!(reset(&pool, user_id).await.is_err());
    }
}
