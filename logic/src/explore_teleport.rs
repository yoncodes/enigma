//! Teleport interactions using the destination in the matching client map.

use anyhow::{Context, Result, ensure};
use database::db::game::explore_runtime;
use sonettobuf::ExploreStep;
use sqlx::SqlitePool;

pub async fn transfer(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit))
    }).context("unknown teleport")?;
    ensure!(unit.kind == 213 && unit.done_effects.is_empty()
        && unit.trigger_effects.iter().all(|effect| matches!(effect.0, 20 | 33)),
        "unsupported teleport effects");
    let destination = unit.special_data.first().and_then(|value| value.as_str())
        .context("teleport destination is missing")?;
    let (x, y) = destination.split_once('#').context("invalid teleport destination")?;
    let x: i32 = x.parse().context("invalid teleport x")?;
    let y: i32 = y.parse().context("invalid teleport y")?;
    let destination_node = layout.node(x, y).context("teleport destination is not on the map")?;
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "teleport area is not open");
    ensure!(destination_node.3 == 0 || stored.state.areas.contains(&destination_node.3),
        "teleport destination area is not open");
    let object = stored.state.objects.get(&object_id).context("teleport state is missing")?;
    ensure!(object.kind == 213 && object.status & 3 == 3 && object.status & 8 == 0,
        "teleport is not available");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "teleport is too far away");
    let direction_matches = match unit.trigger_dir {
        0 => true, 1 => (dx, dy) == (-1, 0), 2 => (dx, dy) == (1, 0),
        3 => (dx, dy) == (0, 1), 4 => (dx, dy) == (0, -1), _ => false,
    };
    ensure!(direction_matches, "teleport cannot be used from this direction");
    for area in &layout.areas {
        if area.id != 0 && !stored.state.areas.contains(&area.id) { continue; }
        for other in &area.units {
            let state = stored.state.objects.get(&other.id).context("map object state is missing")?;
            ensure!(state.kind == other.kind, "map object state does not match its layout");
            if state.status & 1 == 0 { continue; }
            let [left, bottom, right, top] = other.offset_size;
            let covers = (i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(state.x)))
                && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(state.y)));
            if covers {
                let walkable = if matches!(other.kind, 202 | 227) { state.status & 4 != 0 }
                    else { other.default_walkable };
                ensure!(walkable, "teleport destination is blocked");
            }
        }
    }
    // Dynamic objects are not safe to ignore when choosing a landing cell.
    ensure!(!stored.state.objects.iter().any(|(&id, object)| layout.unit(id).is_none()
        && object.status & 1 != 0 && object.x == x && object.y == y),
        "teleport destination has an unsupported dynamic object");
    stored.state.x = x;
    stored.state.y = y;
    stored.state.direction = 0;
    stored.state.visited_nodes.insert((x, y));
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let steps = vec![
        ExploreStep { param: Some(serde_json::json!({"stepType": 8, "x": x, "y": y}).to_string()) },
        crate::explore_engine::position_step(&stored.state),
    ];
    tx.commit().await?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn activated_plate_and_teleport_preserve_both_ends() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 980001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-teleport-test").execute(&pool).await.unwrap();
        let mut state = MapState::new(layout);
        state.x = 20;
        state.y = 1;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();
        assert!(transfer(&pool, user_id, 354).await.is_err());

        let mut tx = pool.begin().await.unwrap();
        let mut saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        saved.state.x = 61;
        saved.state.y = 27;
        let changed = crate::explore_engine::move_client(&mut saved.state, layout, 61, 28).unwrap();
        assert_eq!(changed, vec![206, 354, 355, 360]);
        assert_eq!(saved.state.objects[&354].status & 2, 2);
        assert_eq!(saved.state.objects[&355].status & 2, 2);
        assert_eq!(saved.state.objects[&206].status & 4, 4);
        saved.state.x = 20;
        saved.state.y = 1;
        explore_runtime::save_map(&mut tx, user_id, &mut saved).await.unwrap();
        tx.commit().await.unwrap();

        let steps = transfer(&pool, user_id, 354).await.unwrap();
        assert_eq!(steps.len(), 2);
        let data: serde_json::Value = serde_json::from_str(steps[0].param.as_ref().unwrap()).unwrap();
        assert_eq!((data["stepType"].as_i64(), data["x"].as_i64(), data["y"].as_i64()), (Some(8), Some(65), Some(28)));
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!((saved.state.x, saved.state.y), (65, 28));
        assert!(saved.state.visited_nodes.contains(&(65, 28)));
        tx.commit().await.unwrap();
        assert!(transfer(&pool, user_id, 355).await.is_err(), "wrong side cannot trigger teleport");
    }
}
