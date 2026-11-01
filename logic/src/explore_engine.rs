//! Exploration primitives under development. Not yet wired to the network router.

use config::explore_layout::Layout;
use database::db::game::explore_runtime::MapState;
use sonettobuf::{ExploreInfoNo, ExploreInteractNo, ExploreItemNo, ExploreMapNo, ExploreStep};
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MoveError {
    #[error("map state does not match its layout")]
    WrongMap,
    #[error("movement is not between adjacent map nodes")]
    InvalidStep,
    #[error("movement crosses an unopened area or a height change")]
    UnavailableNode,
    #[error("movement requires unfinished exploration mechanic for object {0}")]
    UnsupportedObject(i32),
    #[error("map object state does not match its layout")]
    InvalidObjectState,
    #[error("invalid or unsupported effect on object {0}")]
    UnsupportedEffect(i32),
    #[error("light network cannot be updated: {0}")]
    LightNetwork(String),
}

fn area_open(state: &MapState, area_id: i32) -> bool {
    area_id == 0 || state.areas.contains(&area_id)
}

pub async fn current_object_kind(
    pool: &sqlx::SqlitePool, user_id: i64, object_id: i32,
) -> anyhow::Result<i32> {
    use anyhow::Context;
    use database::db::game::explore_runtime;
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let state = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    let kind = state.state.objects.get(&object_id).context("unknown exploration object")?.kind;
    tx.commit().await?;
    Ok(kind)
}

/// Move on an ordinary, unobstructed tile. Objects needing interactions are
/// rejected until their server-side mechanics are implemented.
pub fn move_plain(state: &mut MapState, layout: &Layout, x: i32, y: i32) -> Result<(), MoveError> {
    if state.map_id != layout.map_id {
        return Err(MoveError::WrongMap);
    }
    let dx = i64::from(x) - i64::from(state.x);
    let dy = i64::from(y) - i64::from(state.y);
    if dx.abs() + dy.abs() != 1 {
        return Err(MoveError::InvalidStep);
    }
    let from = layout.node(state.x, state.y).ok_or(MoveError::InvalidStep)?;
    let to = layout.node(x, y).ok_or(MoveError::InvalidStep)?;
    if !area_open(state, from.3) || !area_open(state, to.3) || from.2 != to.2 {
        return Err(MoveError::UnavailableNode);
    }

    for area in &layout.areas {
        if !area_open(state, area.id) {
            continue;
        }
        for unit in &area.units {
            let object = state.objects.get(&unit.id).ok_or(MoveError::InvalidObjectState)?;
            if object.kind != unit.kind {
                return Err(MoveError::InvalidObjectState);
            }
            if object.status & 1 == 0 || unit.kind == 0 {
                continue;
            }
            let [left, bottom, right, top] = unit.offset_size;
            let contains = |px: i32, py: i32| {
                let ux = i64::from(px) - i64::from(object.x);
                let uy = i64::from(py) - i64::from(object.y);
                (i64::from(left)..=i64::from(right)).contains(&ux)
                    && (i64::from(bottom)..=i64::from(top)).contains(&uy)
            };
            if contains(state.x, state.y) || contains(x, y) {
                return Err(MoveError::UnsupportedObject(unit.id));
            }
        }
    }
    // Dynamic objects need their own rules, so do not move through one silently.
    for (&id, object) in &state.objects {
        if layout.unit(id).is_none() && object.status & 1 != 0
            && ((object.x, object.y) == (state.x, state.y) || (object.x, object.y) == (x, y))
        {
            return Err(MoveError::UnsupportedObject(id));
        }
    }

    state.x = x;
    state.y = y;
    state.direction = match (dx, dy) {
        (0, 1) => 0,
        (1, 0) => 90,
        (0, -1) => 180,
        (-1, 0) => 270,
        _ => unreachable!(),
    };
    state.visited_nodes.insert((x, y));
    Ok(())
}

pub(crate) fn counter_target(value: &serde_json::Value) -> Option<i32> {
    match value {
        serde_json::Value::Number(number) => i32::try_from(number.as_i64()?).ok(),
        serde_json::Value::String(string) => string.split('#').next()?.parse().ok(),
        _ => None,
    }
}

fn elevator_heights(unit: &config::explore_layout::Unit) -> Result<(i32, i32), MoveError> {
    let value = unit.special_data.first().and_then(|v| v.as_str()).ok_or(MoveError::UnsupportedObject(unit.id))?;
    let (first, second) = value.split_once('#').ok_or(MoveError::UnsupportedObject(unit.id))?;
    let first = first.parse::<i32>().map_err(|_| MoveError::UnsupportedObject(unit.id))?;
    let second = second.parse::<i32>().map_err(|_| MoveError::UnsupportedObject(unit.id))?;
    if first == second { return Err(MoveError::UnsupportedObject(unit.id)); }
    Ok((first, second))
}

fn elevator_height(state: &MapState, unit: &config::explore_layout::Unit) -> Result<i32, MoveError> {
    let (first, second) = elevator_heights(unit)?;
    let object = state.objects.get(&unit.id).ok_or(MoveError::InvalidObjectState)?;
    let value: serde_json::Value = if object.status2.is_empty() { serde_json::json!({}) }
        else { serde_json::from_str(&object.status2).map_err(|_| MoveError::InvalidObjectState)? };
    let data = value.as_object().ok_or(MoveError::InvalidObjectState)?;
    let height = match data.get("height") {
        None => first,
        Some(value) => value.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or(MoveError::InvalidObjectState)?,
    };
    if height != first && height != second { return Err(MoveError::InvalidObjectState); }
    Ok(height)
}

fn set_elevator_height(
    state: &mut MapState, unit: &config::explore_layout::Unit, height: i32, changed: &mut BTreeSet<i32>,
) -> Result<(), MoveError> {
    let (first, second) = elevator_heights(unit)?;
    if height != first && height != second { return Err(MoveError::UnsupportedEffect(unit.id)); }
    if elevator_height(state, unit)? == height { return Ok(()); }
    let object = state.objects.get_mut(&unit.id).ok_or(MoveError::InvalidObjectState)?;
    let mut value: serde_json::Value = if object.status2.is_empty() { serde_json::json!({}) }
        else { serde_json::from_str(&object.status2).map_err(|_| MoveError::InvalidObjectState)? };
    value.as_object_mut().ok_or(MoveError::InvalidObjectState)?.insert("height".into(), height.into());
    object.status2 = serde_json::to_string(&value).map_err(|_| MoveError::InvalidObjectState)?;
    changed.insert(unit.id);
    Ok(())
}

fn node_height(state: &MapState, layout: &Layout, x: i32, y: i32) -> Result<i32, MoveError> {
    let node = layout.node(x, y).ok_or(MoveError::InvalidStep)?;
    let mut height = None;
    for area in &layout.areas {
        if !area_open(state, area.id) { continue; }
        for unit in area.units.iter().filter(|unit| unit.kind == 208) {
            let object = state.objects.get(&unit.id).ok_or(MoveError::InvalidObjectState)?;
            if object.status & 1 == 0 { continue; }
            let [left, bottom, right, top] = unit.offset_size;
            if (i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(object.x)))
                && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(object.y)))
            {
                if height.replace(elevator_height(state, unit)?).is_some() { return Err(MoveError::InvalidObjectState); }
            }
        }
    }
    Ok(height.unwrap_or(node.2))
}

/// The first map's one-use plate changes its own active bit and activates the
/// linked door when every source of that counter is active. Other mechanics
/// remain explicitly unsupported until their behavior has been implemented.
pub fn move_basic(state: &mut MapState, layout: &Layout, x: i32, y: i32) -> Result<Vec<i32>, MoveError> {
    move_basic_with_ignored_unit(state, layout, x, y, None)
}

/// Move the player while a specified object is being moved in the same action.
pub(crate) fn move_basic_with_ignored_unit(state: &mut MapState, layout: &Layout, x: i32, y: i32, ignored: Option<i32>) -> Result<Vec<i32>, MoveError> {
    if state.map_id != layout.map_id {
        return Err(MoveError::WrongMap);
    }
    let dx = i64::from(x) - i64::from(state.x);
    let dy = i64::from(y) - i64::from(state.y);
    if dx.abs() + dy.abs() != 1 {
        return Err(MoveError::InvalidStep);
    }
    let from = layout.node(state.x, state.y).ok_or(MoveError::InvalidStep)?;
    let to = layout.node(x, y).ok_or(MoveError::InvalidStep)?;
    if !area_open(state, from.3) || !area_open(state, to.3)
        || node_height(state, layout, state.x, state.y)? != node_height(state, layout, x, y)? {
        return Err(MoveError::UnavailableNode);
    }

    let mut plate_ids = Vec::new();
    let mut elevator_ids = Vec::new();
    let mut elevator_controls = Vec::new();
    let mut coin_ids = Vec::new();
    let mut save_point = false;
    let mut known_ids = BTreeSet::new();
    for area in &layout.areas {
        for unit in &area.units {
            known_ids.insert(unit.id);
            if Some(unit.id) == ignored { continue; }
            if !area_open(state, area.id) {
                continue;
            }
            let object = state.objects.get(&unit.id).ok_or(MoveError::InvalidObjectState)?;
            if object.kind != unit.kind {
                return Err(MoveError::InvalidObjectState);
            }
            if object.status & 1 == 0 || unit.kind == 0 {
                continue;
            }
            let [left, bottom, right, top] = unit.offset_size;
            let covers = |px: i32, py: i32| {
                let ux = i64::from(px) - i64::from(object.x);
                let uy = i64::from(py) - i64::from(object.y);
                (i64::from(left)..=i64::from(right)).contains(&ux)
                    && (i64::from(bottom)..=i64::from(top)).contains(&uy)
            };
            if !covers(state.x, state.y) && !covers(x, y) {
                continue;
            }
            // Map 301 starts on an inert, non-walkable marker. It prevents
            // entering that tile later but must not prevent leaving spawn.
            if unit.kind == 6 && unit.is_photic && !unit.default_walkable
                && unit.trigger_effects.is_empty() && unit.done_effects.is_empty()
                && covers(state.x, state.y) && !covers(x, y) {
                continue;
            }
            match unit.kind {
                202 | 227 if object.status & 4 != 0 => {}
                205 if unit.default_walkable && unit.special_data.is_empty()
                    && unit.done_effects.is_empty()
                    && unit.trigger_effects.iter().all(|effect| effect.0 == 22) => {}
                // The client animates timed spikes and sends a separate interaction
                // only if the player is caught. Save-point markers are walkable.
                201 | 231 if unit.default_walkable && unit.trigger_effects.is_empty()
                    && unit.done_effects.is_empty() => {
                        if unit.kind == 231 && covers(x, y) && !covers(state.x, state.y) {
                            save_point = true;
                        }
                    }
                // Scene audio markers do not change server-side map state.
                234 if unit.default_walkable && unit.trigger_effects.is_empty()
                    && unit.done_effects.is_empty() => {}
                215 if unit.default_walkable && unit.trigger_effects.is_empty()
                    && unit.done_effects.is_empty() => {}
                // Non-interactive pipe sections are walkable scenery. Their
                // connections are handled by the pipe mechanic separately.
                223 if unit.default_walkable && unit.trigger_effects.is_empty()
                    && unit.done_effects.is_empty() => {}
                206 if unit.default_walkable && unit.trigger_effects.is_empty()
                    && unit.done_effects.is_empty() => {}
                208 if unit.default_walkable && unit.trigger_effects.is_empty() && unit.done_effects.is_empty() => {
                    if covers(x, y) && !covers(state.x, state.y) && object.status & 2 != 0 {
                        elevator_ids.push(unit.id);
                    }
                }
                210 if unit.done_effects.is_empty()
                    && unit.trigger_effects.iter().all(|effect| matches!(effect.0, 2 | 3 | 10 | 19 | 20 | 21 | 31 | 34 | 36)) => {
                        if covers(x, y) && !covers(state.x, state.y) && object.status & 2 != 0 && object.status & 8 == 0 {
                            elevator_controls.push(unit.id);
                        }
                    }
                211 if unit.done_effects.is_empty()
                    && unit.trigger_effects.iter().all(|effect| matches!(effect.0, 3 | 10 | 22)) => {
                        if covers(x, y) && !covers(state.x, state.y)
                            && object.status & 2 != 0 && object.status & 4 == 0 {
                            plate_ids.push(unit.id);
                        }
                    }
                221 if unit.default_walkable
                    && unit.trigger_effects.iter().all(|effect| effect.0 == 20)
                    && unit.done_effects.iter().all(|effect| effect.0 == 1 && effect.1.as_str() == Some("")) => {
                        if covers(x, y) && !covers(state.x, state.y) && object.status & 2 != 0 {
                            coin_ids.push(unit.id);
                        }
                    }
                _ => return Err(MoveError::UnsupportedObject(unit.id)),
            }
        }
    }
    for (&id, object) in &state.objects {
        if !known_ids.contains(&id) && object.status & 1 != 0
            && ((object.x, object.y) == (state.x, state.y) || (object.x, object.y) == (x, y))
        {
            return Err(MoveError::UnsupportedObject(id));
        }
    }

    let mut next = state.clone();
    next.x = x;
    next.y = y;
    next.direction = match (dx, dy) {
        (0, 1) => 0, (1, 0) => 90, (0, -1) => 180, (-1, 0) => 270,
        _ => unreachable!(),
    };
    if save_point { next.reset_anchor = Some((x, y, next.direction)); }
    next.visited_nodes.insert((x, y));
    let mut changed = BTreeSet::new();
    for coin_id in coin_ids {
        next.objects.get_mut(&coin_id).ok_or(MoveError::InvalidObjectState)?.status &= !1;
        changed.insert(coin_id);
    }
    for plate_id in plate_ids {
        activate_once_plate(&mut next, layout, plate_id, &mut changed)?;
    }
    for control_id in elevator_controls {
        let control = layout.unit(control_id).ok_or(MoveError::InvalidObjectState)?;
        for effect in &control.trigger_effects {
            if matches!(effect.0, 10 | 20 | 31 | 34) { continue; }
            if matches!(effect.0, 21 | 36) {
                if effect.1.as_str() != Some("0#1") { return Err(MoveError::UnsupportedEffect(control_id)); }
                let object = next.objects.get_mut(&control_id).ok_or(MoveError::InvalidObjectState)?;
                let old = object.status;
                if effect.0 == 21 { object.status |= 4; } else { object.status |= 8; }
                if old != object.status { changed.insert(control_id); }
                continue;
            }
            if matches!(effect.0, 2 | 3) {
                let mut target_id = counter_target(&effect.1).ok_or(MoveError::UnsupportedEffect(control_id))?;
                if target_id == 0 { target_id = control_id; }
                let target = layout.unit(target_id).ok_or(MoveError::UnsupportedEffect(control_id))?;
                if effect.0 == 2 && target_id != control_id
                    || effect.0 == 3 && !matches!(target.kind, 210 | 213)
                {
                    return Err(MoveError::UnsupportedEffect(control_id));
                }
                let target_state = next.objects.get_mut(&target_id).ok_or(MoveError::InvalidObjectState)?;
                if target_state.kind != target.kind { return Err(MoveError::InvalidObjectState); }
                let previous = target_state.status;
                if effect.0 == 2 { target_state.status &= !2; } else { target_state.status |= 2; }
                if previous != target_state.status { changed.insert(target_id); }
                continue;
            }
            let params = effect.1.as_str().ok_or(MoveError::UnsupportedEffect(control_id))?;
            let mut parts = params.split('#');
            let target_id = parts.next().and_then(|v| v.parse::<i32>().ok()).ok_or(MoveError::UnsupportedEffect(control_id))?;
            let height = parts.next().and_then(|v| v.parse::<i32>().ok()).ok_or(MoveError::UnsupportedEffect(control_id))?;
            let duration = parts.next().and_then(|v| v.parse::<f64>().ok()).ok_or(MoveError::UnsupportedEffect(control_id))?;
            if parts.next().is_some() || !duration.is_finite() || duration <= 0.0 { return Err(MoveError::UnsupportedEffect(control_id)); }
            let target = layout.unit(target_id).ok_or(MoveError::UnsupportedEffect(control_id))?;
            if target.kind != 208 { return Err(MoveError::UnsupportedEffect(control_id)); }
            set_elevator_height(&mut next, target, height, &mut changed)?;
        }
    }
    for elevator_id in elevator_ids {
        let elevator = layout.unit(elevator_id).ok_or(MoveError::InvalidObjectState)?;
        let duration = elevator.special_data.get(1).and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok()).ok_or(MoveError::UnsupportedObject(elevator_id))?;
        if !duration.is_finite() || duration <= 0.0 { return Err(MoveError::UnsupportedObject(elevator_id)); }
        let (first, second) = elevator_heights(elevator)?;
        let height = elevator_height(&next, elevator)?;
        set_elevator_height(&mut next, elevator, if height == first { second } else { first }, &mut changed)?;
    }
    sync_gravity_plates(&mut next, layout, &mut changed)?;
    if next.map_id == 301 && !changed.is_empty() {
        crate::explore_light::sync_light(&mut next, layout, &mut changed)
            .map_err(|error| MoveError::LightNetwork(error.to_string()))?;
    }
    *state = next;
    Ok(changed.into_iter().collect())
}

/// Re-evaluate ordinary pressure plates together after a player or rock moves.
/// Only the configuration used by the first chapter's plates is supported.
pub(crate) fn sync_gravity_plates(
    state: &mut MapState, layout: &Layout, changed: &mut BTreeSet<i32>,
) -> Result<(), MoveError> {
    let mut targets = BTreeSet::new();
    for area in &layout.areas {
        if !area_open(state, area.id) { continue; }
        for unit in area.units.iter().filter(|u| u.kind == 205 && u.special_data.is_empty()
            && u.default_walkable && u.done_effects.is_empty()
            && u.trigger_effects.iter().all(|e| e.0 == 22))
        {
            let object = state.objects.get(&unit.id).ok_or(MoveError::InvalidObjectState)?;
            if object.kind != 205 { return Err(MoveError::InvalidObjectState); }
            if object.status & 3 != 3 { continue; }
            let [left, bottom, right, top] = unit.offset_size;
            let covers = |x: i32, y: i32| {
                (i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(object.x)))
                    && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(object.y)))
            };
            let active = covers(state.x, state.y) || state.objects.iter().any(|(&id, rock)| {
                layout.unit(id).is_none() && rock.kind == 209 && rock.status & 3 == 3 && covers(rock.x, rock.y)
            });
            if active != (object.status & 4 != 0) {
                for effect in &unit.trigger_effects {
                    targets.insert(counter_target(&effect.1).ok_or(MoveError::UnsupportedEffect(unit.id))?);
                }
                let object = state.objects.get_mut(&unit.id).ok_or(MoveError::InvalidObjectState)?;
                if active { object.status |= 4; } else { object.status &= !4; }
                changed.insert(unit.id);
            }
        }
    }
    for target_id in targets {
        let target = layout.unit(target_id).ok_or(MoveError::UnsupportedEffect(target_id))?;
        if !matches!(target.kind, 202 | 227) || !target.done_effects.is_empty()
            || target.trigger_effects.len() != 1 || target.trigger_effects[0].0 != 21
            || target.trigger_effects[0].1.as_str() != Some("0#1") {
            return Err(MoveError::UnsupportedEffect(target_id));
        }
        let mut sources = 0;
        let mut all_active = true;
        for source in layout.areas.iter().flat_map(|area| &area.units) {
            if source.trigger_effects.iter().any(|effect| effect.0 == 22 && counter_target(&effect.1) == Some(target_id)) {
                sources += 1;
                all_active &= state.objects.get(&source.id).ok_or(MoveError::InvalidObjectState)?.status & 4 != 0;
            }
        }
        if sources == 0 { return Err(MoveError::UnsupportedEffect(target_id)); }
        let door = state.objects.get_mut(&target_id).ok_or(MoveError::InvalidObjectState)?;
        if door.kind != target.kind { return Err(MoveError::InvalidObjectState); }
        let old = door.status;
        if all_active { door.status |= 4; } else if target.kind == 202 { door.status &= !4; }
        if old != door.status { changed.insert(target_id); }
    }
    Ok(())
}

fn has_ice(state: &MapState, layout: &Layout, x: i32, y: i32) -> bool {
    layout.areas.iter().filter(|area| area_open(state, area.id)).flat_map(|area| &area.units)
        .any(|unit| {
            if unit.kind != 206 { return false; }
            let Some(object) = state.objects.get(&unit.id) else { return false; };
            let [left, bottom, right, top] = unit.offset_size;
            object.status & 3 == 3
                && (i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(object.x)))
                && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(object.y)))
        })
}

/// The matching client sends the end of an ice slide in one movement request.
/// Simulate each crossed cell and accept only the stop selected by the client.
pub fn move_client(state: &mut MapState, layout: &Layout, x: i32, y: i32) -> Result<Vec<i32>, MoveError> {
    move_client_with_ignored_unit(state, layout, x, y, None)
}

pub(crate) fn move_client_with_ignored_unit(state: &mut MapState, layout: &Layout, x: i32, y: i32, ignored: Option<i32>) -> Result<Vec<i32>, MoveError> {
    let dx = i64::from(x) - i64::from(state.x);
    let dy = i64::from(y) - i64::from(state.y);
    let distance = dx.abs() + dy.abs();
    if distance <= 1 {
        return move_basic_with_ignored_unit(state, layout, x, y, ignored);
    }
    if dx != 0 && dy != 0 || distance > layout.nodes.len() as i64 {
        return Err(MoveError::InvalidStep);
    }
    let step_x = dx.signum() as i32;
    let step_y = dy.signum() as i32;
    let mut next = state.clone();
    let mut changed = BTreeSet::new();
    for step in 1..=distance {
        let px = next.x.checked_add(step_x).ok_or(MoveError::InvalidStep)?;
        let py = next.y.checked_add(step_y).ok_or(MoveError::InvalidStep)?;
        if step < distance && !has_ice(&next, layout, px, py) {
            return Err(MoveError::InvalidStep);
        }
        changed.extend(move_basic_with_ignored_unit(&mut next, layout, px, py, ignored)?);
    }
    if has_ice(&next, layout, x, y) {
        // If the last cell is ice, it is a valid endpoint only when the next
        // cell is actually blocked; unsupported walkable mechanics are errors.
        let px = x.checked_add(step_x).ok_or(MoveError::InvalidStep)?;
        let py = y.checked_add(step_y).ok_or(MoveError::InvalidStep)?;
        let mut probe = next.clone();
        match move_basic_with_ignored_unit(&mut probe, layout, px, py, ignored) {
            Err(MoveError::InvalidStep | MoveError::UnavailableNode) => {}
            Err(MoveError::UnsupportedObject(id)) => {
                let unit = layout.unit(id).ok_or(MoveError::UnsupportedObject(id))?;
                let object = next.objects.get(&id).ok_or(MoveError::InvalidObjectState)?;
                let blocked = if matches!(unit.kind, 202 | 227) {
                    object.status & 4 == 0
                } else {
                    !unit.default_walkable
                };
                if !blocked { return Err(MoveError::UnsupportedObject(id)); }
            }
            Err(error) => return Err(error),
            Ok(_) => return Err(MoveError::InvalidStep),
        }
    }
    *state = next;
    Ok(changed.into_iter().collect())
}

fn activate_once_plate(
    state: &mut MapState, layout: &Layout, plate_id: i32, changed: &mut BTreeSet<i32>,
) -> Result<(), MoveError> {
    let plate = layout.unit(plate_id).ok_or(MoveError::InvalidObjectState)?;
    let mut targets = BTreeSet::new();
    let mut enable = BTreeSet::new();
    for effect in &plate.trigger_effects {
        match effect.0 {
            22 => { targets.insert(counter_target(&effect.1).ok_or(MoveError::UnsupportedEffect(plate_id))?); }
            3 => {
                let id = counter_target(&effect.1).ok_or(MoveError::UnsupportedEffect(plate_id))?;
                let target = layout.unit(id).ok_or(MoveError::UnsupportedEffect(plate_id))?;
                if target.kind != 213 { return Err(MoveError::UnsupportedEffect(plate_id)); }
                enable.insert(id);
            }
            10 => {} // Camera movement is performed by the client.
            _ => return Err(MoveError::UnsupportedEffect(plate_id)),
        }
    }
    state.objects.get_mut(&plate_id).ok_or(MoveError::InvalidObjectState)?.status |= 4;
    changed.insert(plate_id);
    for id in enable {
        let object = state.objects.get_mut(&id).ok_or(MoveError::InvalidObjectState)?;
        if object.kind != 213 { return Err(MoveError::InvalidObjectState); }
        if object.status & 2 == 0 {
            object.status |= 2;
            changed.insert(id);
        }
    }

    for target_id in targets {
        let target = layout.unit(target_id).ok_or(MoveError::UnsupportedEffect(plate_id))?;
        let target_state = state.objects.get(&target_id).ok_or(MoveError::InvalidObjectState)?;
        if target_state.kind != target.kind || !matches!(target.kind, 202 | 227)
            || !target.done_effects.is_empty()
            || target.trigger_effects.len() != 1
            || target.trigger_effects[0].0 != 21
            || target.trigger_effects[0].1.as_str() != Some("0#1")
        {
            return Err(MoveError::UnsupportedEffect(target_id));
        }
        let mut all_active = true;
        for source in layout.areas.iter().flat_map(|area| &area.units) {
            for effect in &source.trigger_effects {
                if effect.0 == 22 && counter_target(&effect.1) == Some(target_id) {
                    let source_state = state.objects.get(&source.id).ok_or(MoveError::InvalidObjectState)?;
                    all_active &= source_state.status & 4 != 0;
                }
            }
        }
        if all_active && target_state.status & 4 == 0 {
            state.objects.get_mut(&target_id).ok_or(MoveError::InvalidObjectState)?.status |= 4;
            changed.insert(target_id);
        }
    }
    Ok(())
}

/// Update-unit steps contain an encoded interaction object inside the outer JSON.
pub fn changed_object_steps(state: &MapState, changed: &[i32]) -> anyhow::Result<Vec<ExploreStep>> {
    changed.iter().map(|id| {
        let object = state.objects.get(id)
            .ok_or_else(|| anyhow::anyhow!("missing changed exploration object"))?;
        let interact = object_message(*id, object);
        Ok(ExploreStep {
            param: Some(serde_json::json!({"stepType": 2, "interact": serde_json::to_string(&interact)?}).to_string()),
        })
    }).collect()
}

/// The client caches this position and uses it to recover after a rejected move.
pub fn position_step(state: &MapState) -> ExploreStep {
    ExploreStep { param: Some(serde_json::json!({
        "stepType": 1, "x": state.x, "y": state.y, "direction": state.direction,
    }).to_string()) }
}

/// Collect an archive and apply its supported self and exit activations.
/// Archives with other server-side effects are rejected until those are supported.
pub async fn collect_basic_archive(
    pool: &sqlx::SqlitePool, user_id: i64, object_id: i32,
) -> anyhow::Result<(i32, Vec<ExploreStep>)> {
    use anyhow::{Context, ensure};
    use database::db::game::explore_runtime;

    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit))
    }).context("unknown exploration object")?;
    ensure!(unit.kind == 228, "object is not an exploration archive");
    ensure!(unit.done_effects.is_empty(), "archive has unsupported done effects");
    let mut self_activation = 0;
    let mut exit_activations = BTreeSet::new();
    let mut counter_targets = BTreeSet::new();
    for effect in &unit.trigger_effects {
        match effect.0 {
            10 | 31 | 35 => {} // Camera, dialogue and hero animation run on the client.
            21 if effect.1.as_str() == Some("0#1") => self_activation += 1,
            22 => {
                let target = counter_target(&effect.1).context("invalid archive counter target")?;
                ensure!(target > 0 && counter_targets.insert(target), "invalid archive counter target");
            }
            21 => {
                let effect_param = effect.1.as_str().context("invalid activation effect")?;
                let (target, value) = effect_param.split_once('#').context("invalid activation effect")?;
                let target: i32 = target.parse().context("invalid activation target")?;
                ensure!(value == "1" && target > 0, "unsupported activation effect");
                let target_unit = layout.unit(target).context("unknown activation target")?;
                ensure!(target_unit.kind == 214 && target_unit.trigger_effects.is_empty()
                    && target_unit.done_effects.is_empty(), "unsupported activation target");
                ensure!(exit_activations.insert(target), "duplicate activation target");
            }
            _ => anyhow::bail!("archive has unsupported trigger effects"),
        }
    }
    ensure!(self_activation == 1, "archive self-activation is missing or ambiguous");
    let archive_id = unit.special_data.first().and_then(counter_target)
        .context("invalid archive configuration")?;
    ensure!(archive_id > 0, "invalid archive identifier");

    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area_open(&stored.state, area.id), "archive area is not open");
    let object = stored.state.objects.get(&object_id).context("archive state is missing")?;
    ensure!(object.kind == 228 && object.status & 3 == 3 && object.status & 12 == 0,
        "archive cannot be collected");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "archive is too far away");
    let direction_matches = match unit.trigger_dir {
        0 => true,
        1 => (dx, dy) == (-1, 0),
        2 => (dx, dy) == (1, 0),
        3 => (dx, dy) == (0, 1),
        4 => (dx, dy) == (0, -1),
        _ => false,
    };
    ensure!(direction_matches, "archive cannot be collected from this direction");
    let mut changed = BTreeSet::from([object_id]);
    stored.state.objects.get_mut(&object_id).context("archive state is missing")?.status |= 4;
    for target_id in counter_targets {
        let target = layout.unit(target_id).context("unknown archive counter target")?;
        ensure!(matches!(target.kind, 202 | 227) && target.done_effects.is_empty()
            && target.trigger_effects.iter().filter(|effect| effect.0 == 21 && effect.1.as_str() == Some("0#1")).count() == 1
            && target.trigger_effects.iter().all(|effect| effect.0 == 10
                || (effect.0 == 21 && effect.1.as_str() == Some("0#1"))),
            "unsupported archive counter target");
        let mut sources = 0;
        let mut all_active = true;
        for source in layout.areas.iter().flat_map(|area| &area.units) {
            if source.trigger_effects.iter().any(|effect| effect.0 == 22 && counter_target(&effect.1) == Some(target_id)) {
                sources += 1;
                all_active &= stored.state.objects.get(&source.id).context("missing archive counter source")?.status & 4 != 0;
            }
        }
        ensure!(sources > 0, "archive counter has no sources");
        if all_active {
            let door = stored.state.objects.get_mut(&target_id).context("missing archive counter target")?;
            ensure!(door.kind == target.kind, "archive counter target has invalid state");
            if door.status & 4 == 0 { door.status |= 4; changed.insert(target_id); }
        }
    }
    for target in exit_activations {
        let object = stored.state.objects.get_mut(&target).context("exit state is missing")?;
        ensure!(object.kind == 214, "exit state does not match its layout");
        if object.status & 4 == 0 {
            object.status |= 4;
            changed.insert(target);
        }
    }
    if map_id == 301 {
        crate::explore_light::sync_light(&mut stored.state, layout, &mut changed)?;
    }

    sqlx::query("INSERT INTO user_explore_chapters (user_id, chapter_id) VALUES (?, ?) \
        ON CONFLICT(user_id, chapter_id) DO NOTHING")
        .bind(user_id).bind(layout.chapter_id).execute(&mut *tx).await?;
    let inserted = sqlx::query("INSERT INTO user_explore_chapter_archives (user_id, chapter_id, archive_id) \
        VALUES (?, ?, ?) ON CONFLICT(user_id, chapter_id, archive_id) DO NOTHING")
        .bind(user_id).bind(layout.chapter_id).bind(archive_id).execute(&mut *tx).await?;
    ensure!(inserted.rows_affected() == 1, "archive was already recorded");
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    steps.push(ExploreStep { param: Some(serde_json::json!({
        "stepType": 16, "archiveId": archive_id,
    }).to_string()) });
    tx.commit().await?;
    Ok((archive_id, steps))
}

fn object_message(id: i32, object: &database::db::game::explore_runtime::ObjectState) -> ExploreInteractNo {
    ExploreInteractNo {
        id: Some(id),
        r#type: Some(object.kind),
        status: Some(object.status),
        status2: Some(object.status2.clone()),
        posx: Some(object.x),
        posy: Some(object.y),
        dir: Some(object.direction),
    }
}

/// Build the full map response expected by the matching client. Inventory is
/// supplied explicitly because it is separate from the map snapshot.
pub fn map_info(
    state: &MapState, first_enter: bool, items: Vec<ExploreItemNo>, use_item_uid: i64,
) -> ExploreInfoNo {
    let interacts = state.objects.iter().map(|(&id, object)| object_message(id, object)).collect();
    let move_nodes = state.visited_nodes.iter()
        .map(|(x, y)| format!("{x}#{y}"))
        .collect::<Vec<_>>().join("|");
    ExploreInfoNo {
        explore_map: Some(ExploreMapNo {
            map_id: Some(state.map_id),
            posx: Some(state.x),
            posy: Some(state.y),
            dir: Some(state.direction),
            interacts,
            area_ids: state.areas.iter().copied().collect(),
            move_nodes: Some(move_nodes),
            is_first_enter: Some(first_enter),
            challenge_count: Some(state.challenge_count),
        }),
        explore_items: items,
        use_item_uid: Some(use_item_uid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_map_reply_includes_saved_position_and_objects() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        move_plain(&mut state, layout, 1, 5).unwrap();
        state.objects.get_mut(&189).unwrap().status = 7;
        let info = map_info(&state, false, vec![], 0);
        let map = info.explore_map.unwrap();
        assert_eq!((map.map_id, map.posx, map.posy, map.dir), (Some(101), Some(1), Some(5), Some(180)));
        assert_eq!(map.interacts.len(), 199);
        let door = map.interacts.iter().find(|object| object.id == Some(189)).unwrap();
        assert_eq!((door.r#type, door.status, door.posx, door.posy), (Some(202), Some(7), Some(13), Some(2)));
        assert_eq!(map.area_ids, vec![1]);
        let nodes = map.move_nodes.unwrap();
        assert!(nodes.contains("1#5") && nodes.contains("1#6"));
        assert_eq!(map.is_first_enter, Some(false));
    }

    #[test]
    fn invalid_move_does_not_change_progress() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        let original = state.clone();
        assert_eq!(move_plain(&mut state, layout, 1, 4), Err(MoveError::InvalidStep));
        assert_eq!(state, original);
        assert_eq!(move_plain(&mut state, layout, 2, 6), Err(MoveError::InvalidStep));
        assert_eq!(state, original);
    }

    #[test]
    fn one_time_dialogue_plate_is_disabled_after_crossing() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 34;
        state.y = 42;
        assert_eq!(move_client(&mut state, layout, 34, 43).unwrap(), vec![549]);
        assert_eq!(state.objects[&549].status & 2, 0);
        move_client(&mut state, layout, 35, 43).unwrap();
        assert!(move_client(&mut state, layout, 34, 43).unwrap().is_empty());
    }

    #[tokio::test]
    async fn first_light_chapter_archive_opens_its_counter_door() {
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 900003_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-light-archive").execute(&pool).await.unwrap();
        let mut state = MapState::new(layout);
        state.x = -5;
        state.y = -1;
        let mut tx = pool.begin().await.unwrap();
        database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        database::db::game::explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        let (archive, _) = collect_basic_archive(&pool, user_id, 303).await.unwrap();
        assert_eq!(archive, 1402101);
        assert!(collect_basic_archive(&pool, user_id, 303).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
        assert_eq!(stored.state.objects[&12].status & 4, 4);
        assert_eq!(stored.state.objects[&303].status & 4, 4);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn pipe_chapter_guide_does_not_overwrite_an_unsupported_archive() {
        let layout = config::explore_layout::layout(201).unwrap().unwrap();
        let mut state = MapState::new(layout);
        let entrance: serde_json::Value = serde_json::from_str(&state.objects[&2].status2).unwrap();
        let pot: serde_json::Value = serde_json::from_str(&state.objects[&17].status2).unwrap();
        assert_eq!((entrance["bindInteractId"].as_i64(), pot["bindInteractId"].as_i64()), (Some(17), Some(2)));
        state.x = 9; state.y = 0;
        let changed = move_basic(&mut state, layout, 10, 0).unwrap();
        assert!(changed.contains(&543));
        assert_eq!(state.objects[&543].status & 12, 12);
        move_basic(&mut state, layout, 10, 1).unwrap();
        assert!(!move_basic(&mut state, layout, 10, 0).unwrap().contains(&543));
        state.x = 14; state.y = -2;
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 900004_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-pipe-archive").execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        database::db::game::explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 201).await.unwrap();
        tx.commit().await.unwrap();
        assert!(collect_basic_archive(&pool, user_id, 355).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 201).await.unwrap().unwrap().state;
        assert_eq!(stored.objects[&534].status & 4, 0);
        assert_eq!(stored.objects[&355].status & 4, 0);
        assert_eq!(stored.objects[&543].status & 12, 12);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn first_plate_opens_its_door_and_survives_reload() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 900001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-plate-test").execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        let initial = MapState::new(layout);
        database::db::game::explore_runtime::insert_map(&mut tx, user_id, &initial).await.unwrap();
        let mut stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        let path = [(1,5),(1,4),(2,4),(2,3),(3,3),(4,3),(4,2),(5,2),(6,2),
            (7,2),(7,3),(8,3),(9,3),(10,3),(11,3),(11,2)];
        let mut last = Vec::new();
        for (x,y) in path {
            last = move_basic(&mut stored.state, layout, x, y).unwrap();
        }
        assert_eq!(last, vec![189, 203]);
        assert_eq!(stored.state.objects[&203].status, 7);
        assert_eq!(stored.state.objects[&189].status, 7);
        let steps = changed_object_steps(&stored.state, &last).unwrap();
        let outer: serde_json::Value = serde_json::from_str(steps[0].param.as_ref().unwrap()).unwrap();
        assert_eq!(outer["stepType"], 2);
        let inner: serde_json::Value = serde_json::from_str(outer["interact"].as_str().unwrap()).unwrap();
        assert_eq!(inner["id"], 189);
        assert_eq!(inner["status"], 7);
        database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut stored).await.unwrap();
        tx.commit().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let restored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!((restored.state.x, restored.state.y), (11, 2));
        assert_eq!(restored.state.objects[&189].status, 7);
        assert_eq!(restored.state.objects[&203].status, 7);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn first_archive_is_recorded_once_and_persists() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 900002_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-archive-test").execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        let mut state = MapState::new(layout);
        for (x, y) in [(1,5),(1,4),(2,4),(2,3),(3,3),(4,3),(4,2),(5,2),(6,2),(7,2)] {
            move_basic(&mut state, layout, x, y).unwrap();
        }
        database::db::game::explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();

        let (archive_id, steps) = collect_basic_archive(&pool, user_id, 515).await.unwrap();
        assert_eq!(archive_id, 140101);
        assert_eq!(steps.len(), 2);
        let archive_step: serde_json::Value = serde_json::from_str(steps[1].param.as_ref().unwrap()).unwrap();
        assert_eq!(archive_step["archiveId"], 140101);
        assert!(collect_basic_archive(&pool, user_id, 515).await.is_err());

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_explore_chapter_archives \
            WHERE user_id = ? AND chapter_id = 1401 AND archive_id = 140101")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        let mut tx = pool.begin().await.unwrap();
        let restored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!(restored.state.objects[&515].status, 7);
        tx.commit().await.unwrap();
    }
}
