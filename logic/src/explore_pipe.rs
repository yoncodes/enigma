//! Single-colour pipe components without memory or multi-colour mixing.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use anyhow::{Context, Result, ensure};
use config::explore_layout::{Layout, Unit};
use database::db::game::explore_runtime::{self, MapState};
use sonettobuf::ExploreStep;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::explore_engine::{changed_object_steps, counter_target, move_basic_with_ignored_unit, position_step};

fn binding(object: &explore_runtime::ObjectState) -> Result<Option<i32>> {
    if object.status2.is_empty() { return Ok(None); }
    let value: serde_json::Value = serde_json::from_str(&object.status2).context("invalid pipe binding data")?;
    let id = match value.get("bindInteractId") {
        Some(value) => value.as_i64().context("invalid pipe binding")?,
        None => 0,
    };
    ensure!(id >= 0, "invalid pipe binding");
    if id == 0 { Ok(None) } else { Ok(Some(i32::try_from(id)?)) }
}

pub(crate) fn carried_pot(state: &MapState, layout: &Layout, selected: i64) -> Result<Option<i32>> {
    let Ok(id) = i32::try_from(selected) else { return Ok(None) };
    let Some(unit) = layout.unit(id) else { return Ok(None) };
    if unit.kind != 224 { return Ok(None); }
    let object = state.objects.get(&id).context("missing carried pot")?;
    ensure!(object.kind == 224 && object.status & 1 != 0 && binding(object)?.is_none(), "invalid carried pot state");
    Ok(Some(id))
}

async fn select_map_object(tx: &mut Transaction<'_, Sqlite>, user: i64, id: i32) -> Result<()> {
    let result = sqlx::query("UPDATE user_explore_runtime_profiles SET use_item_uid = ? WHERE user_id = ?")
        .bind(i64::from(id)).bind(user).execute(&mut **tx).await?;
    ensure!(result.rows_affected() == 1, "missing exploration profile");
    Ok(())
}

fn use_item_step(id: i32) -> ExploreStep {
    ExploreStep { param: Some(serde_json::json!({"stepType":10,"itemUid":id}).to_string()) }
}

fn facing(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<i32> {
    match (i64::from(to_x) - i64::from(from_x), i64::from(to_y) - i64::from(from_y)) {
        (0, 1) => Ok(0), (1, 0) => Ok(90), (0, -1) => Ok(180), (-1, 0) => Ok(270),
        _ => anyhow::bail!("pipe object is not adjacent"),
    }
}

fn direction_delta(dir: i32) -> Result<(i32, i32)> {
    match dir.rem_euclid(360) {
        0 => Ok((0, 1)), 90 => Ok((1, 0)), 180 => Ok((0, -1)), 270 => Ok((-1, 0)),
        _ => anyhow::bail!("pipe direction is not cardinal"),
    }
}

fn outputs(unit: &Unit, direction: i32, incoming: i32) -> Result<Vec<i32>> {
    let shape = unit.special_data.first().and_then(|value| value.as_str()).context("missing pipe shape")?.parse::<i32>()?;
    ensure!((1..=6).contains(&shape), "unsupported pipe shape");
    let raw = (incoming - direction).rem_euclid(360);
    let output = unit.special_data.get(1).and_then(|value| value.as_str()).unwrap_or("");
    if !output.is_empty() {
        let output: i32 = output.parse().context("invalid pipe output direction")?;
        let valid_directions: &[i32] = match shape {
            3 => &[0, 90, 180],
            4 => &[0, 90, 180, 270],
            _ => anyhow::bail!("directed output requires a junction"),
        };
        ensure!(valid_directions.contains(&output), "invalid pipe output direction");
        // A directed junction only accepts inputs on its other connected arms.
        // The client routes these inputs to the configured output arm.
        return Ok(if valid_directions.contains(&raw) && raw != output {
            vec![(output + direction).rem_euclid(360)]
        } else {
            Vec::new()
        });
    }
    let angles = match (shape, raw) {
        (1, 0 | 180) => vec![incoming - 180],
        (2, 0) => vec![incoming + 90], (2, 90) => vec![incoming - 90],
        (3, 0) => vec![incoming + 90, incoming + 180],
        (3, 90) => vec![incoming - 90, incoming + 90],
        (3, 180) => vec![incoming - 90, incoming - 180],
        (4, 0 | 90 | 180 | 270) => vec![incoming + 90, incoming + 180, incoming + 270],
        (5, 0 | 180) => vec![incoming + 90],
        (5, 90 | 270) => vec![incoming - 90],
        (6, 0 | 90 | 180 | 270) => vec![incoming - 180],
        _ => Vec::new(),
    };
    Ok(angles.into_iter().map(|value| value.rem_euclid(360)).collect())
}

fn sync_component(state: &mut MapState, layout: &Layout, id: i32, changed: &mut BTreeSet<i32>) -> Result<()> {
    let mut nodes: BTreeMap<(i32, i32), &Unit> = BTreeMap::new();
    for area in &layout.areas {
        if area.id != 0 && !state.areas.contains(&area.id) { continue; }
        for unit in area.units.iter().filter(|u| matches!(u.kind, 222 | 223 | 225 | 226)) {
            let object = state.objects.get(&unit.id).context("missing pipe object")?;
            ensure!(object.kind == unit.kind, "invalid pipe object");
            if object.status & 1 != 0 {
                ensure!(nodes.insert((object.x, object.y), unit).is_none(), "overlapping pipe objects");
            }
        }
    }
    let selected = state.objects.get(&id).context("missing selected pipe")?;
    let mut component = BTreeSet::from([(selected.x, selected.y)]);
    let mut search = VecDeque::from([(selected.x, selected.y)]);
    while let Some((x,y)) = search.pop_front() {
        for (dx,dy) in [(0,1),(1,0),(0,-1),(-1,0)] {
            let next = (x.checked_add(dx).context("pipe position overflow")?, y.checked_add(dy).context("pipe position overflow")?);
            if nodes.contains_key(&next) && component.insert(next) { search.push_back(next); }
        }
    }
    let mut sources = Vec::new();
    let mut source_colours = BTreeSet::new();
    for pos in &component {
        let unit = nodes[pos];
        match unit.kind {
            222 => {
                let object = &state.objects[&unit.id];
                let binding: serde_json::Value = if object.status2.is_empty() { serde_json::json!({}) }
                    else { serde_json::from_str(&object.status2)? };
                let pot_id = match binding.get("bindInteractId") {
                    Some(value) => value.as_i64().context("invalid pipe pot binding")?,
                    None => 0,
                };
                if pot_id == 0 { continue; }
                let pot_id = i32::try_from(pot_id)?;
                let pot = layout.unit(pot_id).context("missing bound pipe pot")?;
                let pot_state = state.objects.get(&pot_id).context("missing pipe pot state")?;
                ensure!(pot.kind == 224 && pot_state.kind == 224 && pot_state.status & 1 != 0,
                    "invalid bound pipe pot");
                let reverse: serde_json::Value = serde_json::from_str(&pot_state.status2).context("invalid reverse pipe pot binding")?;
                ensure!(reverse.get("bindInteractId").and_then(|value| value.as_i64()) == Some(i64::from(unit.id)),
                    "pipe pot binding is inconsistent");
                let colour = pot.special_data.first().and_then(|v| v.as_str()).context("missing pipe pot colour")?.parse::<i32>()?;
                ensure!(matches!(colour, 1 | 2 | 4), "invalid pipe pot colour");
                source_colours.insert(colour);
                sources.push((unit.id, colour));
            }
            225 => ensure!(unit.special_data.get(1).and_then(|v| v.as_str()).unwrap_or("").is_empty(),
                "pipe sensors with outputs are not supported"),
            226 => anyhow::bail!("pipe memory is not supported"),
            223 => { outputs(unit, state.objects[&unit.id].direction.rem_euclid(360), 0)?; }
            _ => unreachable!(),
        }
    }
    ensure!(source_colours.len() <= 1, "pipe colour mixing is not supported");
    let mut queue = VecDeque::new();
    for (source_id, colour) in sources {
        let source = &state.objects[&source_id];
        let direction = source.direction.rem_euclid(360);
        let (dx,dy) = direction_delta(direction)?;
        let next_pos = (source.x.checked_add(dx).context("pipe position overflow")?,
            source.y.checked_add(dy).context("pipe position overflow")?);
        if let Some(next) = nodes.get(&next_pos) {
            queue.push_back((next.id, (direction + 180).rem_euclid(360), colour));
        }
    }
    let mut visited = BTreeSet::new();
    let mut lit = BTreeSet::new();
    while let Some((id, incoming, colour)) = queue.pop_front() {
        if !visited.insert((id, incoming, colour)) { continue; }
        let unit = layout.unit(id).context("unknown connected pipe")?;
        let object = &state.objects[&id];
        match unit.kind {
            225 => {
                let required = unit.special_data.first().and_then(|v| v.as_str()).context("missing sensor colour")?.parse::<i32>()?;
                if incoming == object.direction.rem_euclid(360) && colour == required { lit.insert(id); }
            }
            223 if object.status & 2 != 0 => {
                for direction in outputs(unit, object.direction.rem_euclid(360), incoming)? {
                    let (dx,dy) = direction_delta(direction)?;
                    let next_pos = (object.x.checked_add(dx).context("pipe position overflow")?,
                        object.y.checked_add(dy).context("pipe position overflow")?);
                    if let Some(next) = nodes.get(&next_pos) {
                        if component.contains(&next_pos) {
                            queue.push_back((next.id, (direction + 180).rem_euclid(360), colour));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut targets = BTreeSet::new();
    for pos in &component {
        let unit = nodes[pos];
        if unit.kind != 225 { continue; }
        let sensor = state.objects.get_mut(&unit.id).context("missing pipe sensor")?;
        let old = sensor.status;
        if lit.contains(&unit.id) { sensor.status |= 4; } else { sensor.status &= !4; }
        if old != sensor.status { changed.insert(unit.id); }
        for effect in &unit.trigger_effects {
            ensure!(effect.0 == 22, "unsupported pipe sensor effect");
            targets.insert(counter_target(&effect.1).context("invalid pipe counter")?);
        }
    }
    for target_id in targets {
        let sources: Vec<_> = layout.areas.iter().flat_map(|a| &a.units)
            .filter(|u| u.trigger_effects.iter().any(|e| e.0 == 22 && counter_target(&e.1) == Some(target_id))).collect();
        ensure!(!sources.is_empty(), "pipe counter has no sources");
        if !sources.iter().all(|u| state.objects.get(&u.id).is_some_and(|object| object.status & 4 != 0)) { continue; }
        let target = layout.unit(target_id).context("missing pipe counter target")?;
        let door = state.objects.get_mut(&target_id).context("missing pipe door state")?;
        if door.status & 4 != 0 { continue; }
        ensure!(target.kind == 227 && door.kind == 227 && target.done_effects.is_empty()
            && target.trigger_effects.iter().filter(|e| e.0 == 21 && e.1.as_str() == Some("0#1")).count() == 1
            && target.trigger_effects.iter().all(|e| e.0 == 10 || (e.0 == 21 && e.1.as_str() == Some("0#1"))),
            "unsupported pipe counter completion");
        door.status |= 4;
        changed.insert(target_id);
    }
    Ok(())
}

pub async fn rotate(pool: &SqlitePool, user_id: i64, object_id: i32, params: &str) -> Result<Vec<ExploreStep>> {
    let multiplier = match params { "27#1" => 1, "27#-1" => -1, _ => anyhow::bail!("invalid pipe rotation") };
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    ensure!(map_id == 201, "this pipe map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown pipe")?;
    ensure!(unit.kind == 223 && unit.done_effects.is_empty()
        && unit.trigger_effects.iter().filter(|e| e.0 == 27 && e.1.as_i64() == Some(90)).count() == 1
        && unit.trigger_effects.iter().all(|e| matches!(e.0, 20 | 27)), "unsupported pipe rotation");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "pipe area is closed");
    let object = stored.state.objects.get(&object_id).context("missing pipe state")?;
    ensure!(object.kind == 223 && object.status & 3 == 3 && object.status & 8 == 0, "pipe is unavailable");
    ensure!((i64::from(stored.state.x) - i64::from(object.x)).abs()
        + (i64::from(stored.state.y) - i64::from(object.y)).abs() == 1, "pipe is not adjacent");
    ensure!(object.direction.rem_euclid(90) == 0, "invalid pipe direction");
    let direction = (object.direction.rem_euclid(360) + multiplier * 90).rem_euclid(360);
    stored.state.objects.get_mut(&object_id).context("missing pipe state")?.direction = direction;
    let mut changed = BTreeSet::from([object_id]);
    sync_component(&mut stored.state, layout, object_id, &mut changed)?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = vec![ExploreStep { param: Some(serde_json::json!({
        "stepType":13,"interactId":object_id,"newDir":direction
    }).to_string()) }];
    steps.extend(changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?);
    tx.commit().await?;
    Ok(steps)
}

/// Pick up a bound pot or insert the carried pot into an empty entrance.
pub async fn interact_entrance(pool: &SqlitePool, user: i64, entrance_id: i32) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user).await?.context("no current exploration map")?;
    ensure!(map_id == 201, "this pipe map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == entrance_id).map(|unit| (area, unit)))
        .context("unknown pipe entrance")?;
    ensure!(unit.kind == 222, "object is not a pipe entrance");
    let mut stored = explore_runtime::load_map(&mut tx, user, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "pipe entrance area is closed");
    let entrance = stored.state.objects.get(&entrance_id).context("missing entrance state")?;
    ensure!(entrance.kind == 222 && entrance.status & 3 == 3 && entrance.status & 8 == 0, "pipe entrance is unavailable");
    let direction = facing(stored.state.x, stored.state.y, entrance.x, entrance.y)?;
    let bound = binding(entrance)?;
    let selected = explore_runtime::use_item_uid(&mut tx, user).await?.context("missing exploration profile")?;
    let carried = carried_pot(&stored.state, layout, selected)?;
    let pot_id = match (bound, carried) {
        (Some(pot), None) => {
            ensure!(layout.unit(pot).is_some_and(|unit| unit.kind == 224), "bound object is not a pipe pot");
            let state = stored.state.objects.get(&pot).context("missing bound pot")?;
            ensure!(state.kind == 224 && state.status & 1 != 0 && binding(state)? == Some(entrance_id), "inconsistent pipe pot binding");
            stored.state.objects.get_mut(&entrance_id).context("missing entrance state")?.status2 = "{\"bindInteractId\":0}".into();
            stored.state.objects.get_mut(&pot).context("missing pot state")?.status2 = "{\"bindInteractId\":0}".into();
            select_map_object(&mut tx, user, pot).await?;
            pot
        }
        (None, Some(pot)) => {
            let state = stored.state.objects.get(&pot).context("missing carried pot")?;
            ensure!(state.kind == 224 && binding(state)?.is_none(), "invalid carried pot state");
            let (x,y) = (entrance.x, entrance.y);
            stored.state.objects.get_mut(&entrance_id).context("missing entrance state")?.status2 = format!("{{\"bindInteractId\":{pot}}}");
            let state = stored.state.objects.get_mut(&pot).context("missing pot state")?;
            state.status2 = format!("{{\"bindInteractId\":{entrance_id}}}");
            state.x = x; state.y = y;
            select_map_object(&mut tx, user, 0).await?;
            pot
        }
        _ => anyhow::bail!("pipe entrance cannot be used in the current state"),
    };
    stored.state.direction = direction;
    let mut changed = BTreeSet::from([entrance_id, pot_id]);
    sync_component(&mut stored.state, layout, entrance_id, &mut changed)?;
    explore_runtime::save_map(&mut tx, user, &mut stored).await?;
    // The entrance update drives the client's pickup/put animation and changes
    // its selected UID, so it must precede the pot update.
    let mut ordered = vec![entrance_id, pot_id];
    ordered.extend(changed.into_iter().filter(|id| *id != entrance_id && *id != pot_id));
    let mut steps = changed_object_steps(&stored.state, &ordered)?;
    steps.push(position_step(&stored.state));
    tx.commit().await?;
    Ok(steps)
}

/// Pick up a free pot, or place the currently carried pot and step backwards.
pub async fn interact_pot(pool: &SqlitePool, user: i64, pot_id: i32, params: Option<&str>) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user).await?.context("no current exploration map")?;
    ensure!(map_id == 201, "this pipe map has unsupported mechanics");
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == pot_id).map(|unit| (area, unit)))
        .context("unknown pipe pot")?;
    ensure!(unit.kind == 224, "object is not a pipe pot");
    let mut stored = explore_runtime::load_map(&mut tx, user, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "pot area is closed");
    let selected = explore_runtime::use_item_uid(&mut tx, user).await?.context("missing exploration profile")?;
    let carried = carried_pot(&stored.state, layout, selected)?;
    let pot = stored.state.objects.get(&pot_id).context("missing pot state")?;
    ensure!(pot.kind == 224 && pot.status & 3 == 3 && pot.status & 8 == 0 && binding(pot)?.is_none(), "pot is unavailable");
    if params.unwrap_or("").is_empty() {
        ensure!(carried.is_none(), "already carrying a pot");
        stored.state.direction = facing(stored.state.x, stored.state.y, pot.x, pot.y)?;
        select_map_object(&mut tx, user, pot_id).await?;
        explore_runtime::save_map(&mut tx, user, &mut stored).await?;
        let steps = vec![position_step(&stored.state), use_item_step(pot_id)];
        tx.commit().await?;
        return Ok(steps);
    }
    ensure!(carried == Some(pot_id), "pot is not being carried");
    let coords = params.unwrap().split('#').map(str::parse::<i32>).collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(coords.len() == 2, "invalid pot placement position");
    let (hero_x, hero_y) = (stored.state.x, stored.state.y);
    let direction = facing(coords[0], coords[1], hero_x, hero_y)?;
    crate::explore_light::can_move_to(&stored.state, layout, pot_id, hero_x, hero_y)?;
    // Unknown pressure-plate types must not be silently bypassed by a dropped pot.
    for plate in layout.areas.iter().flat_map(|area| &area.units).filter(|unit| unit.kind == 205) {
        let object = stored.state.objects.get(&plate.id).context("missing pressure plate")?;
        let [left,bottom,right,top] = plate.offset_size;
        ensure!(object.status & 1 == 0
            || !(i64::from(left)..=i64::from(right)).contains(&(i64::from(hero_x) - i64::from(object.x)))
            || !(i64::from(bottom)..=i64::from(top)).contains(&(i64::from(hero_y) - i64::from(object.y))),
            "pot pressure-plate behavior is not supported");
    }
    let object = stored.state.objects.get_mut(&pot_id).context("missing pot state")?;
    object.x = hero_x; object.y = hero_y;
    let changed = move_basic_with_ignored_unit(&mut stored.state, layout, coords[0], coords[1], Some(pot_id))?;
    ensure!(changed.iter().all(|id| layout.unit(*id).is_some_and(|u| u.kind != 221)), "unsupported coin collection during pot placement");
    stored.state.direction = direction;
    select_map_object(&mut tx, user, 0).await?;
    explore_runtime::save_map(&mut tx, user, &mut stored).await?;
    let mut ids = vec![pot_id];
    ids.extend(changed.into_iter().filter(|id| *id != pot_id));
    let mut steps = changed_object_steps(&stored.state, &ids)?;
    steps.push(position_step(&stored.state));
    steps.push(use_item_step(0));
    tx.commit().await?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directed_junction_routes_one_colour_without_backflow() {
        // Build a tiny circuit from actual map 201 objects, including its
        // directed junction 196 and red-pot source. Both directions are
        // evaluated against the same in-memory state and client configuration.
        let mut layout = config::explore_layout::layout(201).unwrap().unwrap().clone();
        for area in &mut layout.areas {
            area.units.retain(|unit| [2, 15, 17, 18, 196].contains(&unit.id));
        }
        let mut state = MapState::new(&layout);
        for (id, x, y, direction) in [
            (2, 0, 0, 0), (17, 0, 0, 0), (196, 0, 1, 0),
            (15, 1, 1, 270), (18, 2, 1, 0),
        ] {
            let object = state.objects.get_mut(&id).unwrap();
            object.x = x; object.y = y; object.direction = direction;
        }
        let mut changed = BTreeSet::new();
        sync_component(&mut state, &layout, 196, &mut changed).unwrap();
        assert_eq!(state.objects[&15].status & 4, 4, "source reaches output-side sensor");
        assert_eq!(state.objects[&18].status & 4, 4, "sensor opens its configured door");

        // Put the source on the output side: it must not flow backwards to a
        // sensor on an input arm.
        for (id, x, y, direction) in [
            (2, 1, 1, 270), (17, 1, 1, 0), (15, 0, 0, 0),
        ] {
            let object = state.objects.get_mut(&id).unwrap();
            object.x = x; object.y = y; object.direction = direction;
        }
        state.objects.get_mut(&15).unwrap().status &= !4;
        state.objects.get_mut(&18).unwrap().status &= !4;
        changed.clear();
        sync_component(&mut state, &layout, 196, &mut changed).unwrap();
        assert_eq!(state.objects[&15].status & 4, 0, "output does not accept backflow");
        assert_eq!(state.objects[&18].status & 4, 0, "blocked sensor cannot open the door");
    }

    #[tokio::test]
    async fn first_pipe_junction_feeds_two_sensors_and_opens_the_door() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user = 996001;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user).bind("isolated-pipe-puzzle").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(201).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 10; state.y = -11;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user).await.unwrap();
        explore_runtime::insert_map(&mut tx, user, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user, 201).await.unwrap();
        tx.commit().await.unwrap();
        assert!(rotate(&pool, user, 14, "27#2").await.is_err());
        let steps = rotate(&pool, user, 14, "27#-1").await.unwrap();
        let step: serde_json::Value = serde_json::from_str(steps[0].param.as_ref().unwrap()).unwrap();
        assert_eq!((step["stepType"].as_i64(), step["interactId"].as_i64(), step["newDir"].as_i64()),
            (Some(13), Some(14), Some(0)));
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 201).await.unwrap().unwrap().state;
        assert_eq!((state.objects[&15].status & 4, state.objects[&16].status & 4, state.objects[&18].status & 4), (4,4,4));
        tx.commit().await.unwrap();
        rotate(&pool, user, 14, "27#1").await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 201).await.unwrap().unwrap().state;
        assert_eq!((state.objects[&15].status & 4, state.objects[&16].status & 4, state.objects[&18].status & 4), (0,0,4));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn pipe_pot_can_be_carried_placed_and_rebound_without_losing_state() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user = 996002;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user).bind("isolated-pipe-pot").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 47, 1)")
            .bind(user).execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(201).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 10; state.y = -9;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user).await.unwrap();
        explore_runtime::insert_map(&mut tx, user, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user, 201).await.unwrap();
        tx.commit().await.unwrap();
        interact_entrance(&pool, user, 2).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 201).await.unwrap().unwrap().state;
        assert_eq!(explore_runtime::use_item_uid(&mut tx, user).await.unwrap(), Some(17));
        assert_eq!((binding(&state.objects[&2]).unwrap(), binding(&state.objects[&17]).unwrap()), (None, None));
        tx.commit().await.unwrap();
        assert!(crate::explore_entry::change_map(&pool, user, 101).await.is_err());
        assert!(interact_pot(&pool, user, 17, Some("15#-9")).await.is_err());
        interact_pot(&pool, user, 17, Some("11#-9")).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 201).await.unwrap().unwrap().state;
        assert_eq!((state.x,state.y,state.objects[&17].x,state.objects[&17].y), (11,-9,10,-9));
        assert_eq!(explore_runtime::use_item_uid(&mut tx, user).await.unwrap(), Some(0));
        tx.commit().await.unwrap();
        interact_pot(&pool, user, 17, None).await.unwrap();
        crate::explore_move::move_player(&pool, user, 10, -9).await.unwrap();
        interact_entrance(&pool, user, 2).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 201).await.unwrap().unwrap().state;
        assert_eq!((state.objects[&17].x,state.objects[&17].y), (9,-9));
        assert_eq!((binding(&state.objects[&2]).unwrap(), binding(&state.objects[&17]).unwrap()), (Some(17), Some(2)));
        assert_eq!(explore_runtime::use_item_uid(&mut tx, user).await.unwrap(), Some(0));
        tx.commit().await.unwrap();
    }
}
