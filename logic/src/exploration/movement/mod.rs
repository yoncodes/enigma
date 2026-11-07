use std::collections::BTreeSet;

use crate::error::AppError;
use database::db::game::explore::state;
use sqlx::SqlitePool;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleMove {
    pub x: i32,
    pub y: i32,
    pub direction: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MoveOutcome {
    pub steps: Vec<RoleMove>,
}

pub(super) async fn move_to(
    db: &SqlitePool,
    player_id: i64,
    posx: i32,
    posy: i32,
    interact_id: Option<i32>,
) -> Result<MoveOutcome, AppError> {
    if interact_id.is_some() {
        return Err(AppError::InvalidRequest);
    }
    let mut tx = db.begin().await?;
    let map_id = state::last_map(&mut tx, player_id)
        .await?
        .filter(|&id| id != 0)
        .ok_or(AppError::InvalidRequest)?;
    let mut saved = state::load(&mut tx, player_id, map_id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    let map = config::get()
        .explore_map
        .get(map_id)
        .ok_or(AppError::InvalidRequest)?;
    let current = map
        .nodes
        .get(&(saved.posx, saved.posy))
        .ok_or(AppError::InvalidRequest)?;

    let dx = i64::from(posx) - i64::from(saved.posx);
    let dy = i64::from(posy) - i64::from(saved.posy);
    if dx != 0 && dy != 0 {
        return Err(AppError::InvalidRequest);
    }
    let (step_x, step_y, direction, distance) = if dx > 0 {
        (1, 0, 90, dx)
    } else if dx < 0 {
        (-1, 0, 270, -dx)
    } else if dy > 0 {
        (0, 1, 0, dy)
    } else if dy < 0 {
        (0, -1, 180, -dy)
    } else {
        tx.commit().await?;
        return Ok(MoveOutcome { steps: Vec::new() });
    };

    let blocked = blocked_nodes(map, &saved)?;
    let mut steps = Vec::new();
    for offset in 1..=distance {
        let x = i32::try_from(i64::from(saved.posx) + step_x * offset)
            .map_err(|_| AppError::InvalidRequest)?;
        let y = i32::try_from(i64::from(saved.posy) + step_y * offset)
            .map_err(|_| AppError::InvalidRequest)?;
        let node = map.nodes.get(&(x, y)).ok_or(AppError::InvalidRequest)?;
        if node.height != current.height
            || !saved.area_ids.contains(&node.area_id)
            || blocked.contains(&(x, y))
        {
            return Err(AppError::InvalidRequest);
        }
        steps.push(RoleMove { x, y, direction });
    }

    let mut visited = saved
        .move_nodes
        .split('|')
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut known = visited.iter().cloned().collect::<BTreeSet<_>>();
    for step in &steps {
        let key = format!("{}#{}", step.x, step.y);
        if known.insert(key.clone()) {
            visited.push(key);
        }
    }
    saved.posx = posx;
    saved.posy = posy;
    saved.dir = direction;
    saved.move_nodes = visited.join("|");
    if !state::update(&mut tx, player_id, &saved).await? {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;
    Ok(MoveOutcome { steps })
}

fn blocked_nodes(
    map: &config::exploration::Map,
    saved: &database::models::game::explore::state::MapState,
) -> Result<BTreeSet<(i32, i32)>, AppError> {
    let mut blocked = BTreeSet::new();
    for state in saved.interacts.iter().filter(|state| state.status & 1 != 0) {
        let unit = map
            .areas
            .values()
            .flat_map(|area| &area.units)
            .find(|unit| unit.id == state.id)
            .ok_or(AppError::InvalidRequest)?;
        if unit.default_walkable {
            continue;
        }
        for x in unit.offset_size[0]..=unit.offset_size[2] {
            for y in unit.offset_size[1]..=unit.offset_size[3] {
                blocked.insert((
                    state.posx.checked_add(x).ok_or(AppError::InvalidRequest)?,
                    state.posy.checked_add(y).ok_or(AppError::InvalidRequest)?,
                ));
            }
        }
    }
    Ok(blocked)
}
