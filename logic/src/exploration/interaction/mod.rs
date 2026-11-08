use crate::error::AppError;
use database::{
    db::game::explore::state,
    models::game::explore::state::{InteractState, ItemState, MapState},
};
use serde_json::Value;
use sqlx::SqlitePool;

use super::{
    items::ItemEffect,
    movement::{blocked_nodes, record_visited},
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractAction {
    Transfer { x: i32, y: i32, direction: i32 },
    ItemChanged(ItemState),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractOutcome {
    pub id: i32,
    pub action: InteractAction,
}

#[derive(Clone, Copy)]
enum UnitKind {
    Rune,
    Teleport,
}

impl TryFrom<i32> for UnitKind {
    type Error = AppError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            212 => Ok(Self::Rune),
            213 => Ok(Self::Teleport),
            _ => Err(AppError::InvalidRequest),
        }
    }
}

#[derive(Clone, Copy)]
enum TriggerDirection {
    Any,
    Up,
    Down,
}

impl TryFrom<i32> for TriggerDirection {
    type Error = AppError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Any),
            3 => Ok(Self::Up),
            4 => Ok(Self::Down),
            _ => Err(AppError::InvalidRequest),
        }
    }
}

impl TriggerDirection {
    fn trigger_position(self, unit: &config::exploration::Unit) -> Result<(i32, i32), AppError> {
        let x = unit.node_pos[0];
        let y = match self {
            Self::Any => return Err(AppError::InvalidRequest),
            Self::Up => unit.node_pos[1].checked_add(1),
            Self::Down => unit.node_pos[1].checked_sub(1),
        }
        .ok_or(AppError::InvalidRequest)?;
        Ok((x, y))
    }

    fn transfer_direction(self) -> Result<i32, AppError> {
        match self {
            Self::Any => Err(AppError::InvalidRequest),
            Self::Up => Ok(180),
            Self::Down => Ok(0),
        }
    }
}

struct InteractStatus(i32);

impl InteractStatus {
    const ENTERED: i32 = 1;
    const ENABLED: i32 = 2;
    const ACTIVE: i32 = 4;
    const FINISHED: i32 = 8;

    fn can_trigger(self) -> bool {
        self.0 & Self::ENTERED != 0 && self.0 & Self::ENABLED != 0 && self.0 & Self::FINISHED == 0
    }

    fn active(self) -> bool {
        self.0 & Self::ACTIVE != 0
    }
}

#[repr(i32)]
enum RuneStatus {
    Inactive = 0,
    Active = 1,
}

pub(super) async fn interact(
    db: &SqlitePool,
    player_id: i64,
    id: i32,
    params: Option<String>,
) -> Result<InteractOutcome, AppError> {
    if params.is_some() {
        return Err(AppError::InvalidRequest);
    }
    let mut tx = db.begin().await?;
    let map_id = state::last_map(&mut tx, player_id)
        .await?
        .filter(|&map_id| map_id != 0)
        .ok_or(AppError::InvalidRequest)?;
    let mut saved = state::load(&mut tx, player_id, map_id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    let map = config::get()
        .explore_map
        .get(map_id)
        .ok_or(AppError::InvalidRequest)?;
    let unit = map
        .areas
        .values()
        .flat_map(|area| &area.units)
        .find(|unit| unit.id == id)
        .ok_or(AppError::InvalidRequest)?;
    let unit_state = unique_interact(&saved, id)?.clone();
    if unit_state.kind != unit.kind
        || unit_state.posx != unit.node_pos[0]
        || unit_state.posy != unit.node_pos[1]
        || !saved.area_ids.contains(&unit.area_id)
        || !InteractStatus(unit_state.status).can_trigger()
    {
        return Err(AppError::InvalidRequest);
    }

    let action = match UnitKind::try_from(unit.kind)? {
        UnitKind::Teleport => transfer(map, unit, &mut saved)?,
        UnitKind::Rune => activate_item(unit, &unit_state, &mut saved)?,
    };
    if !state::update(&mut tx, player_id, &saved).await? {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;
    Ok(InteractOutcome { id, action })
}

fn unique_interact(saved: &MapState, id: i32) -> Result<&InteractState, AppError> {
    let mut matches = saved.interacts.iter().filter(|state| state.id == id);
    let state = matches.next().ok_or(AppError::InvalidRequest)?;
    if matches.next().is_some() {
        return Err(AppError::InvalidRequest);
    }
    Ok(state)
}

fn transfer(
    map: &config::exploration::Map,
    unit: &config::exploration::Unit,
    saved: &mut MapState,
) -> Result<InteractAction, AppError> {
    let trigger = TriggerDirection::try_from(unit.trigger_dir)?;
    if (saved.posx, saved.posy) != trigger.trigger_position(unit)? {
        return Err(AppError::InvalidRequest);
    }
    let (x, y) = one_pair(&unit.special_datas)?;
    let destination = map.nodes.get(&(x, y)).ok_or(AppError::InvalidRequest)?;
    if !saved.area_ids.contains(&destination.area_id)
        || blocked_nodes(map, saved)?.contains(&(x, y))
    {
        return Err(AppError::InvalidRequest);
    }
    let direction = trigger.transfer_direction()?;
    saved.posx = x;
    saved.posy = y;
    saved.dir = direction;
    record_visited(saved, [(x, y)]);
    Ok(InteractAction::Transfer { x, y, direction })
}

fn activate_item(
    unit: &config::exploration::Unit,
    unit_state: &InteractState,
    saved: &mut MapState,
) -> Result<InteractAction, AppError> {
    if !matches!(
        TriggerDirection::try_from(unit.trigger_dir)?,
        TriggerDirection::Any
    ) || !InteractStatus(unit_state.status).active()
        || (i64::from(saved.posx) - i64::from(unit.node_pos[0])).abs()
            + (i64::from(saved.posy) - i64::from(unit.node_pos[1])).abs()
            != 1
    {
        return Err(AppError::InvalidRequest);
    }
    let item_id = one_i32(&unit.special_datas)?;
    let uid = saved.use_item_uid;
    if uid <= 0 {
        return Err(AppError::InvalidRequest);
    }
    let matches = saved
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.uid == uid)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [index] = matches.as_slice() else {
        return Err(AppError::InvalidRequest);
    };
    let item = &mut saved.items[*index];
    if item.item_id != item_id || item.quantity <= 0 || item.status != RuneStatus::Inactive as i32 {
        return Err(AppError::InvalidRequest);
    }
    ItemEffect::try_from(
        config::get()
            .explore_item
            .get(item.item_id)
            .ok_or(AppError::InvalidRequest)?
            .effect
            .as_str(),
    )?;
    item.status = RuneStatus::Active as i32;
    Ok(InteractAction::ItemChanged(item.clone()))
}

fn one_i32(values: &[Value]) -> Result<i32, AppError> {
    let [Value::String(value)] = values else {
        return Err(AppError::InvalidRequest);
    };
    value.parse().map_err(|_| AppError::InvalidRequest)
}

fn one_pair(values: &[Value]) -> Result<(i32, i32), AppError> {
    let [Value::String(value)] = values else {
        return Err(AppError::InvalidRequest);
    };
    let mut parts = value.split('#');
    let x = parts
        .next()
        .ok_or(AppError::InvalidRequest)?
        .parse()
        .map_err(|_| AppError::InvalidRequest)?;
    let y = parts
        .next()
        .ok_or(AppError::InvalidRequest)?
        .parse()
        .map_err(|_| AppError::InvalidRequest)?;
    if parts.next().is_some() {
        return Err(AppError::InvalidRequest);
    }
    Ok((x, y))
}
