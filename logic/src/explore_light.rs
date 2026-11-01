//! Initial light puzzle: trace the client's eight-direction prism rays.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result, ensure};
use config::explore_layout::{Layout, Unit};
use database::db::game::explore_runtime::{self, MapState};
use sonettobuf::{ExploreItemNo, ExploreStep};
use sqlx::SqlitePool;

use crate::explore_engine::{changed_object_steps, counter_target, move_basic_with_ignored_unit, position_step};

const DIRECTIONS: [(i32, i32); 8] = [(0,1),(1,1),(1,0),(1,-1),(0,-1),(-1,-1),(-1,0),(-1,1)];

fn trace_light(state: &MapState, layout: &Layout) -> Result<BTreeSet<i32>> {
    let nodes: BTreeSet<(i32, i32)> = layout.nodes.iter().map(|n| (n.0, n.1)).collect();
    let mut at: BTreeMap<(i32, i32), Vec<&Unit>> = BTreeMap::new();
    let mut prisms = BTreeMap::new();
    let mut sources = BTreeSet::new();
    for area in &layout.areas {
        if area.id != 0 && !state.areas.contains(&area.id) { continue; }
        for unit in &area.units {
            let object = state.objects.get(&unit.id).context("missing light map object")?;
            ensure!(object.kind == unit.kind, "invalid light map object");
            if object.status & 1 == 0 || unit.kind == 0 { continue; }
            if unit.kind == 215 { sources.insert((object.x, object.y)); }
            if matches!(unit.kind, 218 | 219) && object.status & 2 != 0 { prisms.insert(unit.id, unit); }
            let [left, bottom, right, top] = unit.offset_size;
            ensure!(right >= left && top >= bottom && i64::from(right) - i64::from(left) <= 64
                && i64::from(top) - i64::from(bottom) <= 64, "invalid light object extent");
            for ox in left..=right {
                for oy in bottom..=top {
                    let x = object.x.checked_add(ox).context("light object position overflow")?;
                    let y = object.y.checked_add(oy).context("light object position overflow")?;
                    at.entry((x,y)).or_default().push(unit);
                }
            }
        }
    }
    let mut light_balls = Vec::new();
    let mut rock_positions = BTreeSet::new();
    for (&id, object) in &state.objects {
        if layout.unit(id).is_some() || object.status & 1 == 0 { continue; }
        match object.kind {
            216 => light_balls.push((object.x, object.y)),
            209 => { rock_positions.insert((object.x, object.y)); }
            _ => anyhow::bail!("unsupported dynamic object in light puzzle"),
        }
    }
    ensure!(light_balls.len() <= 1, "multiple portable light sources are not supported");
    let mut queue: VecDeque<i32> = prisms.iter().filter_map(|(&id, _)| {
        let object = &state.objects[&id];
        (sources.contains(&(object.x, object.y)) || light_balls.iter().any(|&(x,y)|
            (i64::from(x) - i64::from(object.x)).abs() + (i64::from(y) - i64::from(object.y)).abs() <= 1)).then_some(id)
    }).collect();
    let mut emitted = BTreeSet::new();
    let mut receivers = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !emitted.insert(id) { continue; }
        let prism = prisms[&id];
        let object = &state.objects[&id];
        ensure!(object.direction.rem_euclid(45) == 0, "invalid prism direction");
        let direction = object.direction.rem_euclid(360);
        let directions = if prism.kind == 219 {
            vec![direction - 45, direction + 45]
        } else { vec![direction] };
        for angle in directions {
            let angle = angle.rem_euclid(360);
            let (dx, dy) = DIRECTIONS[(angle / 45) as usize];
            let (mut x, mut y) = (object.x, object.y);
            while nodes.contains(&(x,y)) {
                x = x.checked_add(dx).context("light position overflow")?;
                y = y.checked_add(dy).context("light position overflow")?;
                if rock_positions.contains(&(x,y)) { break; }
                let Some(here) = at.get(&(x,y)) else { continue };
                let mut blockers = here.iter().filter(|unit| {
                    let object = &state.objects[&unit.id];
                    if matches!(unit.kind, 202 | 227) {
                        object.status & 4 == 0
                    } else {
                        !unit.is_photic
                    }
                });
                let Some(blocker) = blockers.next() else { continue };
                ensure!(blockers.next().is_none(), "ambiguous overlapping light obstacles");
                if matches!(blocker.kind, 218 | 219) && prisms.contains_key(&blocker.id) {
                    queue.push_back(blocker.id);
                } else if blocker.kind == 220 {
                    let receiver = &state.objects[&blocker.id];
                    let incoming = (angle + 180).rem_euclid(360);
                    let both = blocker.special_data.first().and_then(|v| v.as_str()) == Some("1");
                    let receiver_dir = receiver.direction.rem_euclid(360);
                    if incoming == receiver_dir
                        || both && incoming == (receiver_dir + 180).rem_euclid(360) {
                        receivers.insert(blocker.id);
                    }
                }
                break;
            }
        }
    }
    Ok(receivers)
}

pub(crate) fn sync_light(state: &mut MapState, layout: &Layout, changed: &mut BTreeSet<i32>) -> Result<()> {
    // Doors of this type latch open. Re-trace after an opening because the
    // open door itself can stop blocking a light beam.
    for _ in 0..=layout.areas.iter().map(|a| a.units.len()).sum::<usize>() {
        let lit = trace_light(state, layout)?;
        let mut targets = BTreeSet::new();
        for area in &layout.areas {
            if area.id != 0 && !state.areas.contains(&area.id) { continue; }
            for unit in area.units.iter().filter(|u| u.kind == 220) {
                let object = state.objects.get_mut(&unit.id).context("missing light receiver")?;
                if object.status & 1 == 0 { continue; }
                let old = object.status;
                if lit.contains(&unit.id) { object.status |= 4; } else { object.status &= !4; }
                if old != object.status { changed.insert(unit.id); }
                for effect in &unit.trigger_effects {
                    ensure!(effect.0 == 22, "unsupported light receiver effect");
                    targets.insert(counter_target(&effect.1).context("invalid light counter target")?);
                }
            }
        }
        let mut opened = false;
        for id in targets {
            let mut count = 0;
            let mut full = true;
            for source in layout.areas.iter().flat_map(|a| &a.units) {
                if source.trigger_effects.iter().any(|e| e.0 == 22 && counter_target(&e.1) == Some(id)) {
                    count += 1;
                    full &= state.objects.get(&source.id).context("missing light counter source")?.status & 4 != 0;
                }
            }
            if count == 0 || !full { continue; }
            let target = layout.unit(id).context("missing light counter target")?;
            let door = state.objects.get_mut(&id).context("missing light counter state")?;
            if door.status & 4 != 0 { continue; }
            ensure!(target.kind == 227 && door.kind == 227
                && target.trigger_effects.iter().filter(|e| e.0 == 21 && e.1.as_str() == Some("0#1")).count() == 1
                && target.trigger_effects.iter().all(|e| e.0 == 10 || (e.0 == 21 && e.1.as_str() == Some("0#1")))
                && target.done_effects.iter().all(|e| e.0 == 21 && e.1.as_str() == Some("0#1")),
                "unsupported light counter completion");
            door.status |= 4;
            changed.insert(id);
            opened = true;
        }
        if !opened { return Ok(()); }
    }
    anyhow::bail!("light network did not stabilize")
}

/// A client rotation carries a signed one-step multiplier, not an absolute angle.
pub async fn rotate(pool: &SqlitePool, user_id: i64, object_id: i32, params: &str) -> Result<Vec<ExploreStep>> {
    let multiplier = match params { "27#1" => 1, "27#-1" => -1, _ => anyhow::bail!("invalid prism rotation") };
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    ensure!(map_id == 301, "this light map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown prism")?;
    ensure!(matches!(unit.kind, 218 | 219) && unit.done_effects.is_empty()
        && unit.trigger_effects.iter().filter(|e| e.0 == 27 && e.1.as_i64() == Some(45)).count() == 1
        && unit.trigger_effects.iter().all(|e| matches!(e.0, 20 | 27)), "unsupported prism configuration");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "prism area is closed");
    let object = stored.state.objects.get(&object_id).context("missing prism state")?;
    ensure!(object.kind == unit.kind && object.status & 3 == 3 && object.status & 8 == 0, "prism is unavailable");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "prism is too far away");
    ensure!(object.direction.rem_euclid(45) == 0, "invalid prism direction");
    let direction = (object.direction.rem_euclid(360) + multiplier * 45).rem_euclid(360);
    stored.state.objects.get_mut(&object_id).context("missing prism state")?.direction = direction;
    let mut changed = BTreeSet::from([object_id]);
    sync_light(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = vec![ExploreStep { param: Some(serde_json::json!({
        "stepType":13,"interactId":object_id,"newDir":direction
    }).to_string()) }];
    steps.extend(changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?);
    tx.commit().await?;
    Ok(steps)
}

pub(crate) fn can_move_to(state: &MapState, layout: &Layout, ignored: i32, x: i32, y: i32) -> Result<()> {
    let node = layout.node(x, y).context("prism move destination is not on the map")?;
    ensure!(node.3 == 0 || state.areas.contains(&node.3), "prism move destination is in a closed area");
    for area in &layout.areas {
        if area.id != 0 && !state.areas.contains(&area.id) { continue; }
        for unit in &area.units {
            if unit.id == ignored || unit.kind == 0 { continue; }
            let object = state.objects.get(&unit.id).context("missing map object")?;
            if object.status & 1 == 0 { continue; }
            let [left, bottom, right, top] = unit.offset_size;
            if !(i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(object.x)))
                || !(i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(object.y))) { continue; }
            let walkable = if matches!(unit.kind, 202 | 227) { object.status & 4 != 0 } else { unit.default_walkable };
            ensure!(walkable && unit.can_use_item, "prism move destination is blocked");
        }
    }
    ensure!(!state.objects.iter().any(|(&id, object)| layout.unit(id).is_none()
        && object.status & 1 != 0 && (object.x, object.y) == (x,y)), "prism move destination is occupied");
    Ok(())
}

/// Push or pull a movable prism by one cell; the player moves with it.
pub async fn move_prism(pool: &SqlitePool, user_id: i64, object_id: i32, x: i32, y: i32) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    ensure!(map_id == 301, "this light map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown prism")?;
    ensure!(matches!(unit.kind, 218 | 219) && unit.can_move && unit.offset_size == [0,0,0,0],
        "object is not a movable prism");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "prism area is closed");
    let object = stored.state.objects.get(&object_id).context("missing prism state")?;
    ensure!(object.kind == unit.kind && object.status & 3 == 3 && object.status & 8 == 0, "prism is unavailable");
    ensure!((i64::from(stored.state.x) - i64::from(object.x)).abs()
        + (i64::from(stored.state.y) - i64::from(object.y)).abs() == 1, "prism is not adjacent");
    let dx = i64::from(x) - i64::from(object.x);
    let dy = i64::from(y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() == 1, "prism must move one cell");
    let hero_x = i32::try_from(i64::from(stored.state.x) + dx)?;
    let hero_y = i32::try_from(i64::from(stored.state.y) + dy)?;
    can_move_to(&stored.state, layout, object_id, x, y)?;
    can_move_to(&stored.state, layout, object_id, hero_x, hero_y)?;
    let object = stored.state.objects.get_mut(&object_id).context("missing prism state")?;
    object.x = x; object.y = y;
    let mut changed: BTreeSet<i32> = move_basic_with_ignored_unit(&mut stored.state, layout, hero_x, hero_y, Some(object_id))?
        .into_iter().collect();
    // Coins are not permitted under the move destinations; no reward can be lost.
    ensure!(changed.iter().all(|id| layout.unit(*id).is_some_and(|u| u.kind != 221)), "unsupported coin collection during prism move");
    changed.insert(object_id);
    sync_light(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = vec![position_step(&stored.state)];
    steps.extend(changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?);
    tx.commit().await?;
    Ok(steps)
}

/// Place or recall the reserved portable light source. Its item stays in the backpack.
pub async fn use_light_ball(pool: &SqlitePool, user_id: i64, uid: i64, x: i32, y: i32) -> Result<Vec<ExploreStep>> {
    let id = i32::try_from(uid).context("invalid light source identifier")?;
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    ensure!(map_id == 301, "this light map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    ensure!(layout.unit(id).is_none(), "light source identifier conflicts with the map");
    let item = explore_runtime::list_items(&mut tx, user_id).await?.into_iter()
        .find(|item| item.uid == Some(uid)).context("portable light source is not owned")?;
    ensure!(item.item_id == Some(150003) && item.quantity == Some(1) && item.status == Some(0),
        "invalid portable light source");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    let present = stored.state.objects.get(&id).is_some_and(|object| object.status & 1 != 0);
    if present {
        ensure!(x == 0 && y == 0 && stored.state.objects[&id].kind == 216, "invalid light source recall");
        stored.state.objects.get_mut(&id).context("missing light source")?.status &= !1;
    } else {
        if let Some(old) = stored.state.objects.get(&id) { ensure!(old.kind == 216, "invalid light source state"); }
        ensure!((i64::from(x) - i64::from(stored.state.x)).abs()
            + (i64::from(y) - i64::from(stored.state.y)).abs() == 1, "light source must be placed in an adjacent cell");
        ensure!(!stored.state.objects.iter().any(|(&other, object)| other != id && layout.unit(other).is_none()
            && object.kind == 216 && object.status & 1 != 0), "another light source is already present");
        can_move_to(&stored.state, layout, id, x, y)?;
        stored.state.objects.insert(id, explore_runtime::ObjectState {
            kind: 216, status: 3, status2: String::new(), x, y, direction: 0,
        });
    }
    let mut changed = BTreeSet::from([id]);
    sync_light(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    tx.commit().await?;
    Ok(steps)
}

pub struct PrismRepair {
    pub item: ExploreItemNo,
    pub steps: Vec<ExploreStep>,
}

/// Consume one configured repair tool and enable the adjacent broken prism.
pub async fn repair_prism(pool: &SqlitePool, user_id: i64, uid: i64, object_id: i32) -> Result<PrismRepair> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    ensure!(map_id == 301, "this light map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown broken prism")?;
    ensure!(matches!(unit.kind, 218 | 219) && unit.special_data.first().and_then(|v| v.as_str()) == Some("150004"),
        "unsupported prism repair");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "prism area is closed");
    let object = stored.state.objects.get(&object_id).context("missing prism state")?;
    ensure!(object.kind == unit.kind && object.status & 1 != 0 && object.status & 2 == 0 && object.status & 8 == 0,
        "prism does not need repair");
    ensure!((i64::from(stored.state.x) - i64::from(object.x)).abs()
        + (i64::from(stored.state.y) - i64::from(object.y)).abs() == 1, "prism is too far away");
    let removed = sqlx::query("DELETE FROM user_explore_runtime_items WHERE user_id = ? AND uid = ? \
        AND item_id = 150004 AND quantity = 1 AND status = 0")
        .bind(user_id).bind(uid).execute(&mut *tx).await?;
    ensure!(removed.rows_affected() == 1, "repair tool is not available");
    stored.state.objects.get_mut(&object_id).context("missing prism state")?.status |= 2;
    let mut changed = BTreeSet::from([object_id]);
    sync_light(&mut stored.state, layout, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    tx.commit().await?;
    Ok(PrismRepair { item: ExploreItemNo { uid: Some(uid), item_id: Some(150004), quantity: Some(0), status: Some(0) }, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn first_light_puzzle_traces_both_sources_and_latches_the_door() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995001;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-light-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = -5; map.y = 4;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        assert!(rotate(&pool, user_id, 298, "27#2").await.is_err());
        rotate(&pool, user_id, 298, "27#-1").await.unwrap();
        let first = rotate(&pool, user_id, 298, "27#-1").await.unwrap();
        let step: serde_json::Value = serde_json::from_str(first[0].param.as_ref().unwrap()).unwrap();
        assert_eq!((step["stepType"].as_i64(), step["interactId"].as_i64(), step["newDir"].as_i64()),
            (Some(13), Some(298), Some(0)));
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.objects[&136].status & 4, map.objects[&297].status & 4, map.objects[&118].status & 4), (4,4,4));
        tx.commit().await.unwrap();
        rotate(&pool, user_id, 298, "27#1").await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.objects[&136].status & 4, map.objects[&118].status & 4), (0,4));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn light_chapter_route_reaches_its_first_prism_and_door() {
        crate::init_test_server_config();
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(data.to_str().unwrap()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995002;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-light-route").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let map = MapState::new(layout);
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        for (x,y) in [(-6,-5),(-6,-4),(-6,-3),(-6,-2),(-5,-2),(-5,-1)] {
            crate::explore_move::move_player(&pool, user_id, x, y).await.unwrap();
        }
        crate::explore_engine::collect_basic_archive(&pool, user_id, 303).await.unwrap();
        for (x,y) in [(-6,-1),(-6,0),(-7,0),(-7,1),(-7,2),(-7,3),(-7,4),(-6,4),(-5,4)] {
            crate::explore_move::move_player(&pool, user_id, x, y).await
                .unwrap_or_else(|e| panic!("light route ({x},{y}): {e:#}"));
        }
        rotate(&pool, user_id, 298, "27#-1").await.unwrap();
        rotate(&pool, user_id, 298, "27#-1").await.unwrap();
        for (x,y) in [(-6,4),(-6,5),(-6,6),(-6,7),(-6,8),(-7,8),(-7,9),(-7,10),(-7,11),
            (-7,12),(-8,12),(-9,12),(-10,12),(-11,12),(-12,12),(-13,12),(-14,12),(-15,12),
            (-16,12),(-17,12),(-18,12),(-18,13),(-18,14),(-18,15),(-18,16),(-18,17),
            (-17,17),(-17,18),(-17,19),(-17,20),(-16,20)] {
            crate::explore_move::move_player(&pool, user_id, x, y).await
                .unwrap_or_else(|e| panic!("second light route ({x},{y}): {e:#}"));
        }
        for _ in 0..2 { rotate(&pool, user_id, 307, "27#-1").await.unwrap(); }
        for (x,y) in [(-17,20),(-18,20),(-19,20)] {
            crate::explore_move::move_player(&pool, user_id, x, y).await.unwrap();
        }
        for _ in 0..3 { rotate(&pool, user_id, 308, "27#1").await.unwrap(); }
        for (x,y) in [(-18,20),(-17,20),(-16,20),(-15,20),(-15,19),(-15,18)] {
            crate::explore_move::move_player(&pool, user_id, x, y).await.unwrap();
        }
        for _ in 0..2 { rotate(&pool, user_id, 309, "27#1").await.unwrap(); }
        let mut tx = pool.begin().await.unwrap();
        let stored = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!(stored.objects[&12].status & 4, 4);
        assert_eq!(stored.objects[&118].status & 4, 4);
        assert_eq!((stored.objects[&310].status & 4, stored.objects[&305].status & 4), (4,4));
        assert_eq!((stored.x, stored.y, stored.objects[&298].direction), (-15,18,0));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn movable_prism_push_and_pull_keep_player_and_light_state_together() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995003;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-light-movement").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = 0; map.y = 21;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        assert!(move_prism(&pool, user_id, 176, 3, 21).await.is_err());
        move_prism(&pool, user_id, 176, 2, 21).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.x, map.y, map.objects[&176].x, map.objects[&176].y), (1,21,2,21));
        tx.commit().await.unwrap();
        move_prism(&pool, user_id, 176, 1, 21).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.x, map.y, map.objects[&176].x, map.objects[&176].y), (0,21,1,21));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn light_chain_crosses_a_transparent_wall_and_turns_diagonally() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995004;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-diagonal-light").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = -16; map.y = 20;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        for _ in 0..2 { rotate(&pool, user_id, 307, "27#-1").await.unwrap(); }
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
        map.state.x = -19; map.state.y = 20;
        explore_runtime::save_map(&mut tx, user_id, &mut map).await.unwrap();
        tx.commit().await.unwrap();
        for _ in 0..3 { rotate(&pool, user_id, 308, "27#1").await.unwrap(); }
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
        map.state.x = -15; map.state.y = 18;
        explore_runtime::save_map(&mut tx, user_id, &mut map).await.unwrap();
        tx.commit().await.unwrap();
        for _ in 0..2 { rotate(&pool, user_id, 309, "27#1").await.unwrap(); }
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.objects[&310].status & 4, map.objects[&305].status & 4), (4,4));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn portable_light_is_reserved_and_can_be_recalled() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995005;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-portable-light").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = -15; map.y = 19;
        map.objects.get_mut(&309).unwrap().direction = 90;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        let uid = explore_runtime::grant_item(&mut tx, user_id, 150003, 1).await.unwrap()[0].uid.unwrap();
        tx.commit().await.unwrap();
        assert!(use_light_ball(&pool, user_id, uid, -16, 18).await.is_err());
        use_light_ball(&pool, user_id, uid, -15, 18).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.objects[&(uid as i32)].kind, map.objects[&(uid as i32)].status & 1), (216,1));
        assert_eq!((map.objects[&310].status & 4, map.objects[&305].status & 4), (4,4));
        assert_eq!(explore_runtime::list_items(&mut tx, user_id).await.unwrap()[0].quantity, Some(1));
        tx.commit().await.unwrap();
        use_light_ball(&pool, user_id, uid, 0, 0).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!((map.objects[&(uid as i32)].status & 1, map.objects[&310].status & 4, map.objects[&305].status & 4), (0,0,4));
        assert_eq!(explore_runtime::list_items(&mut tx, user_id).await.unwrap()[0].quantity, Some(1));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn repair_tool_is_consumed_only_when_a_broken_prism_is_fixed() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 995006;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-prism-repair").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = 22; map.y = 18;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        let uid = explore_runtime::grant_item(&mut tx, user_id, 150004, 1).await.unwrap()[0].uid.unwrap();
        tx.commit().await.unwrap();
        assert!(repair_prism(&pool, user_id, uid, 194).await.is_err());
        let result = repair_prism(&pool, user_id, uid, 181).await.unwrap();
        assert_eq!(result.item.quantity, Some(0));
        assert!(repair_prism(&pool, user_id, uid, 181).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert_eq!(map.objects[&181].status & 3, 3);
        assert!(explore_runtime::list_items(&mut tx, user_id).await.unwrap().is_empty());
        tx.commit().await.unwrap();
    }
}
