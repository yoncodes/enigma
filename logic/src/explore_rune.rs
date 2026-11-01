//! Rune selection and energy transfer primitives; not yet network-wired.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use config::explore_layout::{Layout, Unit};
use database::db::game::explore_runtime::{self, MapState};
use sonettobuf::{ExploreItemNo, ExploreStep};
use sqlx::SqlitePool;

use crate::explore_engine::{changed_object_steps, counter_target};

fn counter_sources(layout: &Layout, target: i32) -> Result<BTreeSet<i32>> {
    let mut sources = BTreeSet::new();
    for unit in layout.areas.iter().flat_map(|area| &area.units) {
        for effect in &unit.trigger_effects {
            if effect.0 == 22 {
                let effect_target = counter_target(&effect.1).context("invalid counter target")?;
                if effect_target == target {
                    sources.insert(unit.id);
                }
            }
        }
    }
    Ok(sources)
}

fn counter_is_full(state: &MapState, layout: &Layout, target: i32) -> Result<bool> {
    let sources = counter_sources(layout, target)?;
    ensure!(!sources.is_empty(), "counter has no sources");
    for id in sources {
        if state.objects.get(&id).context("counter source state is missing")?.status & 4 == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn simple_counter_door(unit: &Unit) -> bool {
    matches!(unit.kind, 202 | 227) && unit.done_effects.is_empty()
        && unit.trigger_effects.len() == 1
        && unit.trigger_effects[0].0 == 21
        && unit.trigger_effects[0].1.as_str() == Some("0#1")
}

/// Default map data describes object bits, while the client recalculates
/// counters on startup. Initialize supported doors whose sources start active.
pub fn initialize_counter_doors(state: &mut MapState, layout: &Layout) -> Result<()> {
    ensure!(state.map_id == layout.map_id, "wrong exploration layout");
    let mut targets = BTreeSet::new();
    for unit in layout.areas.iter().flat_map(|area| &area.units) {
        for effect in &unit.trigger_effects {
            if effect.0 == 22 {
                targets.insert(counter_target(&effect.1).context("invalid counter target")?);
            }
        }
    }
    for target_id in targets {
        let Some(target) = layout.unit(target_id) else { continue };
        if simple_counter_door(target) && counter_is_full(state, layout, target_id)? {
            state.objects.get_mut(&target_id).context("counter target state is missing")?.status |= 4;
        }
    }
    Ok(())
}

/// Select or deselect an owned rune; the client consumes the use-item step.
pub async fn select(pool: &SqlitePool, user_id: i64, uid: i64) -> Result<ExploreStep> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    let selected = explore_runtime::use_item_uid(&mut tx, user_id).await?.context("profile is missing")?;
    ensure!(crate::explore_pipe::carried_pot(&stored.state, layout, selected)?.is_none(), "cannot select a rune while carrying a pot");
    let items = explore_runtime::list_items(&mut tx, user_id).await?;
    let item = items.iter().find(|item| item.uid == Some(uid)).context("exploration item is not owned")?;
    let config = config::explore_rewards::item(item.item_id.context("missing item id")?)?
        .context("unknown exploration item")?;
    ensure!(config.effect == "2", "item is not a rune");
    let current = explore_runtime::use_item_uid(&mut tx, user_id).await?.context("profile is missing")?;
    let selected = if current == uid { 0 } else { uid };
    explore_runtime::select_item(&mut tx, user_id, selected).await?;
    tx.commit().await?;
    Ok(ExploreStep { param: Some(serde_json::json!({"stepType": 10, "itemUid": selected}).to_string()) })
}

pub struct RuneSwap {
    pub item: ExploreItemNo,
    pub steps: Vec<ExploreStep>,
}

/// Awaken a selected rune from a lit pillar, or use it to awaken a pillar.
/// An awakened rune and an already-lit pillar both retain their state.
pub async fn swap(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<RuneSwap> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit))
    }).context("unknown rune object")?;
    ensure!(unit.kind == 212 && unit.done_effects.is_empty()
        && unit.trigger_effects.iter().all(|effect| effect.0 == 22), "unsupported rune effects");
    let required_item = unit.special_data.first().and_then(counter_target).context("invalid rune item")?;
    let uid = explore_runtime::use_item_uid(&mut tx, user_id).await?.context("profile is missing")?;
    let item = explore_runtime::list_items(&mut tx, user_id).await?.into_iter()
        .find(|item| item.uid == Some(uid)).context("selected rune is missing")?;
    ensure!(item.item_id == Some(required_item), "selected rune does not match the object");
    let item_config = config::explore_rewards::item(required_item)?.context("unknown rune item")?;
    ensure!(item_config.effect == "2", "selected item is not a rune");
    let old_item_status = item.status.context("missing rune status")?;
    ensure!(matches!(old_item_status, 0 | 1), "invalid rune status");

    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "rune area is not open");
    let object = stored.state.objects.get(&object_id).context("rune state is missing")?;
    ensure!(object.kind == 212 && object.status & 3 == 3 && object.status & 8 == 0,
        "rune cannot be used");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "rune is too far away");
    let direction_matches = match unit.trigger_dir {
        0 => true, 1 => (dx,dy) == (-1,0), 2 => (dx,dy) == (1,0),
        3 => (dx,dy) == (0,1), 4 => (dx,dy) == (0,-1), _ => false,
    };
    ensure!(direction_matches, "rune cannot be used from this direction");
    let unit_active = object.status & 4 != 0;
    ensure!(unit_active != (old_item_status == 1), "rune and pillar are already in the same state");
    let mut changed = BTreeSet::new();
    if !unit_active {
        stored.state.objects.get_mut(&object_id).context("rune state is missing")?.status |= 4;
        changed.insert(object_id);
    }
    let mut shown_areas = BTreeSet::new();
    for effect in unit.trigger_effects.iter().filter(|_| !unit_active) {
        let target = counter_target(&effect.1).context("invalid counter target")?;
        let target_unit = layout.unit(target).context("unknown counter target")?;
        let is_full = counter_is_full(&stored.state, layout, target)?;
        if simple_counter_door(target_unit) {
            let target_state = stored.state.objects.get_mut(&target).context("counter target state is missing")?;
            let before = target_state.status;
            if is_full {
                target_state.status |= 4;
            } else if target_unit.kind == 202 {
                target_state.status &= !4;
            }
            if before != target_state.status { changed.insert(target); }
        } else if target_unit.kind == 6 && target_unit.done_effects.is_empty()
            && target_unit.trigger_effects.iter().all(|effect| matches!(effect.0, 10 | 11 | 33))
        {
            if is_full {
                for action in &target_unit.trigger_effects {
                    if action.0 == 11 {
                        let area_id = counter_target(&action.1).context("invalid area id")?;
                        ensure!(layout.areas.iter().any(|area| area.id == area_id), "unknown area id");
                        if area_id != 0 && stored.state.areas.insert(area_id) {
                            shown_areas.insert(area_id);
                        }
                    }
                }
            }
        } else {
            anyhow::bail!("unsupported rune counter target");
        }
    }
    let item = explore_runtime::update_item_status(&mut tx, user_id, uid, old_item_status, 1).await?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    // The client accepts one newly opened area per step.
    for area_id in shown_areas {
        steps.push(ExploreStep { param: Some(serde_json::json!({"stepType": 14, "showAreas": [area_id]}).to_string()) });
    }
    tx.commit().await?;
    Ok(RuneSwap { item, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn relocate(pool: &SqlitePool, user_id: i64, x: i32, y: i32) {
        let mut tx = pool.begin().await.unwrap();
        let mut state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        state.state.x = x;
        state.state.y = y;
        explore_runtime::save_map(&mut tx, user_id, &mut state).await.unwrap();
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn awakened_rune_and_pillars_remain_active_and_open_the_linked_area() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 940001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-rune-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        initialize_counter_doors(&mut state, layout).unwrap();
        assert_eq!(state.objects[&529].status & 4, 4);
        state.x = 32;
        state.y = 19;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        let item = explore_runtime::grant_item(&mut tx, user_id, 150002, 1).await.unwrap();
        let uid = item[0].uid.unwrap();
        tx.commit().await.unwrap();
        select(&pool, user_id, uid).await.unwrap();
        let result = swap(&pool, user_id, 208).await.unwrap();
        assert_eq!(result.item.status, Some(1));
        assert!(swap(&pool, user_id, 208).await.is_err());
        relocate(&pool, user_id, 17, -1).await;
        assert_eq!(swap(&pool, user_id, 210).await.unwrap().item.status, Some(1));
        relocate(&pool, user_id, 21, -1).await;
        let result = swap(&pool, user_id, 211).await.unwrap();
        assert_eq!(result.item.status, Some(1));
        assert!(result.steps.iter().any(|step| step.param.as_deref().unwrap_or("").contains("showAreas")));
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
        assert!(saved.areas.contains(&1001));
        assert_eq!(saved.objects[&208].status & 4, 4);
        assert_eq!(saved.objects[&210].status & 4, 4);
        assert_eq!(saved.objects[&211].status & 4, 4);
        assert_eq!(saved.objects[&529].status & 4, 4);
        let items = explore_runtime::list_items(&mut tx, user_id).await.unwrap();
        assert_eq!(items[0].status, Some(1));
        tx.commit().await.unwrap();
    }
}
