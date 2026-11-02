//! Timed traps are animated by the client; each trap specifies its return tile.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use database::db::game::explore_runtime;
use sonettobuf::ExploreStep;
use sqlx::SqlitePool;

use crate::explore_engine::{changed_object_steps, position_step, sync_gravity_plates};

pub async fn trigger(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown trap")?;
    ensure!(unit.kind == 201 && unit.default_walkable && unit.trigger_effects.is_empty()
        && unit.done_effects.is_empty(), "unsupported trap configuration");
    let return_data = unit.special_data.first().and_then(|v| v.as_str()).context("missing trap return position")?;
    let values = return_data.split('#').map(str::parse::<i32>).collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(values.len() == 3 && matches!(values[2], 0 | 90 | 180 | 270), "invalid trap return position");
    let node = layout.node(values[0], values[1]).context("trap return position is not on the map")?;
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!((area.id == 0 || stored.state.areas.contains(&area.id))
        && (node.3 == 0 || stored.state.areas.contains(&node.3)), "trap or return area is closed");
    let object = stored.state.objects.get(&object_id).context("missing trap state")?;
    ensure!(object.kind == 201 && object.status & 3 == 3, "trap is unavailable");
    let [left, bottom, right, top] = unit.offset_size;
    ensure!((i64::from(left)..=i64::from(right)).contains(&(i64::from(stored.state.x) - i64::from(object.x)))
        && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(stored.state.y) - i64::from(object.y))),
        "player is not on the trap");
    stored.state.x = values[0];
    stored.state.y = values[1];
    stored.state.direction = values[2];
    stored.state.visited_nodes.insert((values[0], values[1]));
    stored.state.challenge_count = stored.state.challenge_count.checked_add(1).context("challenge count overflow")?;
    let mut changed = BTreeSet::new();
    sync_gravity_plates(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    steps.push(ExploreStep { param: Some(serde_json::json!({
        "stepType":7,"triggerInteractId":object_id,"x":values[0],"y":values[1]
    }).to_string()) });
    steps.push(position_step(&stored.state));
    tx.commit().await?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn configured_trap_return_and_challenge_count_survive_reload() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 993001;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-trap-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(102).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = 9; map.y = 3;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
        tx.commit().await.unwrap();
        assert!(trigger(&pool, user_id, 20).await.is_err());
        crate::explore_move::move_player(&pool, user_id, 10, 3).await.unwrap();
        let steps = trigger(&pool, user_id, 20).await.unwrap();
        let data: serde_json::Value = serde_json::from_str(steps[steps.len() - 2].param.as_ref().unwrap()).unwrap();
        assert_eq!((data["stepType"].as_i64(), data["triggerInteractId"].as_i64(), data["x"].as_i64(), data["y"].as_i64()),
            (Some(7), Some(20), Some(1), Some(2)));
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap().state;
        assert_eq!((map.x, map.y, map.direction, map.challenge_count), (1, 2, 90, 1));
        tx.commit().await.unwrap();
        assert!(trigger(&pool, user_id, 20).await.is_err());
    }
}
