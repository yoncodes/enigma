//! The first chapter's portable rock, stored atomically with map progress.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use config::explore_layout::{Layout, Unit};
use database::db::game::explore_runtime::{self, MapState, ObjectState};
use sonettobuf::{ExploreItemNo, ExploreStep};
use sqlx::SqlitePool;

use crate::explore_engine::{changed_object_steps, sync_gravity_plates};

const ROCK_ITEM: i32 = 150001;
const ROCK_KIND: i32 = 209;
const PLACED_STATUS: i32 = -1;
const ROCK_UID_STRIDE: i64 = 1_000_000;
const ROCK_UID_FIRST_SLOT: i64 = 1000;

// The engine derives a rock's originating map and table from its identifier.
// The quotient provides distinct slots for rock identifiers.
fn table_code(map_id: i32, table_id: i32) -> Result<i64> {
    ensure!((1..1000).contains(&map_id) && (1..1000).contains(&table_id), "unsupported stone table identifier");
    Ok(i64::from(map_id) * 1000 + i64::from(table_id))
}

fn rock_source(uid: i64) -> Option<(i32, i32)> {
    if uid < ROCK_UID_STRIDE * ROCK_UID_FIRST_SLOT || uid > i64::from(i32::MAX) { return None; }
    let code = uid % ROCK_UID_STRIDE;
    let (map_id, table_id) = ((code / 1000) as i32, (code % 1000) as i32);
    if map_id == 0 || table_id == 0 { return None; }
    Some((map_id, table_id))
}

async fn allocate_rock_uid(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, user_id: i64,
    state: &MapState, table_id: i32) -> Result<i64> {
    let code = table_code(state.map_id, table_id)?;
    // A previous map may contain a tombstone or placed object with this UID.
    let saved: Vec<String> = sqlx::query_scalar("SELECT state_json FROM user_explore_runtime_maps WHERE user_id = ?")
        .bind(user_id).fetch_all(&mut **tx).await?;
    let mut occupied = BTreeSet::new();
    for json in saved {
        let map: MapState = serde_json::from_str(&json).context("invalid saved exploration state")?;
        occupied.extend(map.objects.keys().copied());
    }
    occupied.extend(state.objects.keys().copied());
    for slot in ROCK_UID_FIRST_SLOT..=(i64::from(i32::MAX) / ROCK_UID_STRIDE) {
        let uid = slot * ROCK_UID_STRIDE + code;
        if uid > i64::from(i32::MAX) || occupied.contains(&(uid as i32)) { continue; }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_explore_runtime_items WHERE uid = ?)")
            .bind(uid).fetch_one(&mut **tx).await?;
        if !exists { return Ok(uid); }
    }
    anyhow::bail!("stone table identifier slots exhausted")
}

fn set_table_rock(object: &mut ObjectState, placed: bool) {
    // status is client-visible gameplay state, not an ownership association.
    object.status2 = serde_json::json!({"status": i32::from(placed)}).to_string();
}

async fn update_bound_table(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, user_id: i64,
    state: &mut MapState, uid: i64, placed: bool, changed: &mut BTreeSet<i32>) -> Result<()> {
    let Some((map_id, id)) = rock_source(uid) else { return Ok(()) };
    let Some(layout) = config::explore_layout::layout(map_id)? else { return Ok(()) };
    if !layout.unit(id).is_some_and(|unit| unit.kind == 217) { return Ok(()); }
    if map_id == state.map_id {
        let table = state.objects.get_mut(&id).context("origin stone table is missing")?;
        ensure!(table.kind == 217 && table.status & 4 != 0, "invalid origin stone table");
        set_table_rock(table, placed);
        changed.insert(id);
    } else {
        let mut origin = explore_runtime::load_map(tx, user_id, map_id).await?.context("origin exploration map is missing")?;
        let table = origin.state.objects.get_mut(&id).context("origin stone table is missing")?;
        ensure!(table.kind == 217 && table.status & 4 != 0, "invalid origin stone table");
        set_table_rock(table, placed);
        explore_runtime::save_map(tx, user_id, &mut origin).await?;
    }
    Ok(())
}

pub struct RockChange {
    pub item: ExploreItemNo,
    pub steps: Vec<ExploreStep>,
}

/// Dispatch by the owned item's configured effect, including the valid (0, 0) tile.
pub async fn item_effect(pool: &SqlitePool, user_id: i64, uid: i64) -> Result<String> {
    let mut tx = pool.begin().await?;
    let item = explore_runtime::list_items(&mut tx, user_id).await?.into_iter()
        .find(|item| item.uid == Some(uid)).context("exploration item is not in the backpack")?;
    let effect = config::explore_rewards::item(item.item_id.context("item id is missing")?)?
        .context("unknown exploration item")?.effect.clone();
    tx.commit().await?;
    Ok(effect)
}

fn covers(unit: &Unit, x: i32, y: i32, object: &ObjectState) -> bool {
    let [left, bottom, right, top] = unit.offset_size;
    (i64::from(left)..=i64::from(right)).contains(&(i64::from(x) - i64::from(object.x)))
        && (i64::from(bottom)..=i64::from(top)).contains(&(i64::from(y) - i64::from(object.y)))
}

fn can_place(state: &MapState, layout: &Layout, x: i32, y: i32) -> Result<()> {
    let dx = (i64::from(x) - i64::from(state.x)).abs();
    let dy = (i64::from(y) - i64::from(state.y)).abs();
    ensure!(dx <= 1 && dy <= 1 && dx + dy > 0, "rock destination is outside the adjacent cells");
    let node = layout.node(x, y).context("rock destination is not a map node")?;
    ensure!(node.3 == 0 || state.areas.contains(&node.3), "rock destination is in a closed area");
    for area in &layout.areas {
        if area.id != 0 && !state.areas.contains(&area.id) { continue; }
        for unit in &area.units {
            let object = state.objects.get(&unit.id).context("map object state is missing")?;
            ensure!(object.kind == unit.kind, "map object state does not match the layout");
            if unit.kind != 0 && object.status & 1 != 0 && covers(unit, x, y, object) {
                let walkable = if matches!(unit.kind, 202 | 227) {
                    object.status & 4 != 0
                } else {
                    unit.default_walkable
                };
                ensure!(unit.can_use_item && walkable, "rock destination is blocked");
            }
        }
    }
    ensure!(!state.objects.iter().any(|(&id, object)| layout.unit(id).is_none()
        && object.status & 1 != 0 && (object.x, object.y) == (x, y)), "rock destination is occupied");
    Ok(())
}

/// Take a rock, or recall its placed rock to its original table.
pub async fn take(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<RockChange> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown stone table")?;
    ensure!(unit.kind == 217 && unit.special_data.len() == 1
        && unit.special_data[0].as_str() == Some("150001") && unit.done_effects.is_empty()
        && unit.trigger_effects.iter().all(|effect| effect.0 == 20), "unsupported stone table");
    let config = config::explore_rewards::item(ROCK_ITEM)?.context("missing rock configuration")?;
    ensure!(!config.is_stackable && config.effect == "1#209", "unsupported rock configuration");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "stone table is in a closed area");
    let object = stored.state.objects.get(&object_id).context("missing stone table state")?;
    ensure!(object.kind == 217 && object.status & 3 == 3 && object.status & 8 == 0,
        "stone table is unavailable");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "stone table is too far away");
    ensure!(match unit.trigger_dir {
        0 => true, 1 => (dx, dy) == (-1, 0), 2 => (dx, dy) == (1, 0),
        3 => (dx, dy) == (0, 1), 4 => (dx, dy) == (0, -1), _ => false,
    }, "stone table cannot be used from this direction");
    if object.status & 4 != 0 {
        let code = table_code(map_id, object_id)?;
        let matches: Vec<i64> = sqlx::query_scalar("SELECT uid FROM user_explore_runtime_items WHERE user_id = ? AND item_id = ? AND quantity = 1 AND map_id = ? AND uid >= ? AND uid % ? = ?")
            .bind(user_id).bind(ROCK_ITEM).bind(map_id).bind(ROCK_UID_STRIDE * ROCK_UID_FIRST_SLOT).bind(ROCK_UID_STRIDE).bind(code)
            .fetch_all(&mut *tx).await?;
        ensure!(matches.len() == 1, "stone table rock identity is missing or ambiguous");
        let uid = matches[0];
        let rock_id = i32::try_from(uid).context("invalid bound rock UID")?;
        let saved: Vec<(i32, String)> = sqlx::query_as("SELECT map_id, state_json FROM user_explore_runtime_maps WHERE user_id = ?")
            .bind(user_id).fetch_all(&mut *tx).await?;
        let mut locations = Vec::new();
        for (saved_id, json) in saved {
            let state: MapState = serde_json::from_str(&json).context("invalid saved exploration state")?;
            ensure!(state.map_id == saved_id, "exploration map state mismatch");
            if state.objects.get(&rock_id).is_some_and(|rock| rock.kind == ROCK_KIND && rock.status & 1 != 0) {
                locations.push(saved_id);
            }
        }
        ensure!(locations.len() == 1, "bound rock placement is missing or ambiguous");
        let mut changed = BTreeSet::from([object_id]);
        if locations[0] == map_id {
            stored.state.objects.get_mut(&rock_id).context("bound rock is missing")?.status &= !1;
            changed.insert(rock_id);
        } else {
            let mut placed_map = explore_runtime::load_map(&mut tx, user_id, locations[0]).await?.context("rock map is missing")?;
            placed_map.state.objects.get_mut(&rock_id).context("bound rock is missing")?.status &= !1;
            let other_layout = config::explore_layout::layout(locations[0])?.context("rock layout is missing")?;
            let mut other_changed = BTreeSet::from([rock_id]);
            sync_gravity_plates(&mut placed_map.state, other_layout, &mut other_changed)?;
            if locations[0] == 301 { crate::explore_light::sync_light(&mut placed_map.state, other_layout, &mut other_changed)?; }
            explore_runtime::save_map(&mut tx, user_id, &mut placed_map).await?;
        }
        let deleted = sqlx::query("DELETE FROM user_explore_runtime_items WHERE user_id = ? AND uid = ? AND item_id = ? AND quantity = 1 AND status = ?")
            .bind(user_id).bind(uid).bind(ROCK_ITEM).bind(PLACED_STATUS).execute(&mut *tx).await?;
        ensure!(deleted.rows_affected() == 1, "bound rock is no longer owned or placed");
        let table = stored.state.objects.get_mut(&object_id).context("stone table is missing")?;
        table.status &= !4;
        table.status2.clear();
        sync_gravity_plates(&mut stored.state, layout, &mut changed)?;
        if map_id == 301 { crate::explore_light::sync_light(&mut stored.state, layout, &mut changed)?; }
        explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
        let steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
        tx.commit().await?;
        return Ok(RockChange { item: ExploreItemNo { uid: Some(uid), item_id: Some(ROCK_ITEM), quantity: Some(0), status: Some(0) }, steps });
    }
    let uid = allocate_rock_uid(&mut tx, user_id, &stored.state, object_id).await?;
    sqlx::query("INSERT INTO user_explore_runtime_items (uid, user_id, item_id, quantity, status) VALUES (?, ?, ?, 1, 0)")
        .bind(uid).bind(user_id).bind(ROCK_ITEM).execute(&mut *tx).await?;
    let item = ExploreItemNo { uid: Some(uid), item_id: Some(ROCK_ITEM), quantity: Some(1), status: Some(0) };
    let table = stored.state.objects.get_mut(&object_id).context("stone table is missing")?;
    table.status |= 4;
    set_table_rock(table, false);
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &[object_id])?;
    let first_hint = sqlx::query("INSERT OR IGNORE INTO user_explore_stone_hints (user_id, map_id, unit_id) VALUES (?, ?, ?)")
        .bind(user_id).bind(map_id).bind(object_id).execute(&mut *tx).await?.rows_affected() == 1;
    if first_hint {
        steps.push(ExploreStep { param: Some(serde_json::json!({"stepType":4,"materialData":[
            serde_json::json!({"materilType":15,"materilId":ROCK_ITEM,"quantity":1}).to_string()
        ]}).to_string()) });
    }
    tx.commit().await?;
    Ok(RockChange { item, steps })
}

/// Place a rock. A sentinel keeps the owned UID while it is absent from the backpack.
pub async fn place(pool: &SqlitePool, user_id: i64, uid: i64, x: i32, y: i32) -> Result<RockChange> {
    let id = i32::try_from(uid).context("invalid rock identifier")?;
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    ensure!(layout.unit(id).is_none(), "rock identifier conflicts with the map");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    can_place(&stored.state, layout, x, y)?;
    if let Some(previous) = stored.state.objects.get(&id) {
        ensure!(previous.kind == ROCK_KIND && previous.status & 1 == 0, "rock is already on the map");
    }
    let mut item = explore_runtime::update_item_status(&mut tx, user_id, uid, 0, PLACED_STATUS).await?;
    ensure!(item.item_id == Some(ROCK_ITEM) && item.quantity == Some(1), "item is not an available rock");
    stored.state.objects.insert(id, ObjectState { kind: ROCK_KIND, status: 3, status2: String::new(), x, y, direction: 0 });
    let mut changed = BTreeSet::from([id]);
    update_bound_table(&mut tx, user_id, &mut stored.state, uid, true, &mut changed).await?;
    sync_gravity_plates(&mut stored.state, layout, &mut changed)?;
    if map_id == 301 { crate::explore_light::sync_light(&mut stored.state, layout, &mut changed)?; }
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    item.quantity = Some(0);
    item.status = Some(0);
    tx.commit().await?;
    Ok(RockChange { item, steps })
}

/// Pick up the current map's placed rock, releasing any plate it held.
pub async fn pick_up(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<RockChange> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    ensure!(layout.unit(object_id).is_none(), "object is not a portable rock");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?.context("missing map state")?;
    let rock = stored.state.objects.get(&object_id).context("rock is not on this map")?;
    ensure!(rock.kind == ROCK_KIND && rock.status & 3 == 3, "rock is unavailable");
    let dx = (i64::from(stored.state.x) - i64::from(rock.x)).abs();
    let dy = (i64::from(stored.state.y) - i64::from(rock.y)).abs();
    ensure!(dx + dy <= 1, "rock is too far away");
    stored.state.objects.get_mut(&object_id).context("missing rock")?.status &= !1;
    let item = explore_runtime::update_item_status(&mut tx, user_id, i64::from(object_id), PLACED_STATUS, 0).await?;
    ensure!(item.item_id == Some(ROCK_ITEM) && item.quantity == Some(1), "rock ownership is invalid");
    let mut changed = BTreeSet::from([object_id]);
    update_bound_table(&mut tx, user_id, &mut stored.state, i64::from(object_id), false, &mut changed).await?;
    sync_gravity_plates(&mut stored.state, layout, &mut changed)?;
    if map_id == 301 { crate::explore_light::sync_light(&mut stored.state, layout, &mut changed)?; }
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let steps = changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    tx.commit().await?;
    Ok(RockChange { item, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup(user_id: i64, x: i32, y: i32) -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind(format!("isolated-rock-{user_id}"))
            .execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(102).unwrap().unwrap();
        assert!(layout.node(x, y).is_some());
        let mut state = MapState::new(layout);
        state.x = x;
        state.y = y;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
        tx.commit().await.unwrap();
        pool
    }

    async fn set_position(pool: &SqlitePool, user_id: i64, x: i32, y: i32) {
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap();
        map.state.x = x;
        map.state.y = y;
        explore_runtime::save_map(&mut tx, user_id, &mut map).await.unwrap();
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn rock_and_pressure_plate_are_saved_and_rollback_together() {
        let user = 992001;
        let pool = setup(user, 5, 12).await;
        let item = take(&pool, user, 29).await.unwrap().item;
        assert!(take(&pool, user, 29).await.is_err());
        let uid = item.uid.unwrap();
        assert_eq!(item_effect(&pool, user, uid).await.unwrap(), "1#209");
        set_position(&pool, user, 5, 5).await;
        assert!(place(&pool, user, uid, 5, 5).await.is_err());

        sqlx::query("CREATE TRIGGER rock_rollback BEFORE UPDATE ON user_explore_runtime_maps \
            BEGIN SELECT RAISE(ABORT, 'simulated failure'); END")
            .execute(&pool).await.unwrap();
        assert!(place(&pool, user, uid, 6, 5).await.is_err());
        sqlx::query("DROP TRIGGER rock_rollback").execute(&pool).await.unwrap();
        assert_eq!(item_effect(&pool, user, uid).await.unwrap(), "1#209");

        let placed = place(&pool, user, uid, 6, 5).await.unwrap();
        assert_eq!(placed.item.quantity, Some(0));
        assert!(item_effect(&pool, user, uid).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        assert_eq!((map.state.objects[&14].status & 4, map.state.objects[&2].status & 4), (4, 4));
        assert_eq!((map.state.objects[&(uid as i32)].x, map.state.objects[&(uid as i32)].y), (6, 5));
        assert!(explore_runtime::list_items(&mut tx, user).await.unwrap().is_empty());
        tx.commit().await.unwrap();

        let retrieved = pick_up(&pool, user, uid as i32).await.unwrap();
        assert_eq!(retrieved.item.uid, Some(uid));
        assert_eq!(retrieved.item.quantity, Some(1));
        assert!(pick_up(&pool, user, uid as i32).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        assert_eq!((map.state.objects[&14].status & 4, map.state.objects[&2].status & 4), (0, 0));
        assert_eq!(map.state.objects[&(uid as i32)].status & 1, 0);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn stone_table_recalls_its_placed_rock_and_can_grant_it_again() {
        let user = 992004;
        let pool = setup(user, 5, 12).await;
        let first = take(&pool, user, 29).await.unwrap();
        assert!(first.steps.iter().any(|s| serde_json::from_str::<serde_json::Value>(s.param.as_ref().unwrap()).unwrap()["stepType"] == 4));
        let uid = first.item.uid.unwrap();
        set_position(&pool, user, 5, 5).await;
        place(&pool, user, uid, 6, 5).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap().state;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&state.objects[&29].status2).unwrap()["status"], 1);
        assert_eq!(state.objects[&14].status & 4, 4);
        tx.commit().await.unwrap();
        set_position(&pool, user, 5, 12).await;
        let recalled = take(&pool, user, 29).await.unwrap();
        assert_eq!(recalled.item.uid, Some(uid));
        assert_eq!(recalled.item.quantity, Some(0));
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap().state;
        assert_eq!(state.objects[&29].status & 4, 0);
        assert!(state.objects[&29].status2.is_empty());
        assert_eq!(state.objects[&(uid as i32)].status & 1, 0);
        assert_eq!(state.objects[&14].status & 4, 0);
        tx.commit().await.unwrap();
        let next = take(&pool, user, 29).await.unwrap();
        assert!(!next.steps.iter().any(|s| serde_json::from_str::<serde_json::Value>(s.param.as_ref().unwrap()).unwrap()["stepType"] == 4));
        assert_ne!(next.item.uid, Some(uid));
        assert_eq!(next.item.quantity, Some(1));
    }

    #[tokio::test]
    async fn recall_does_not_remove_another_tables_rock() {
        let user = 992006;
        let pool = setup(user, 5, 12).await;
        let first = take(&pool, user, 29).await.unwrap().item.uid.unwrap();
        set_position(&pool, user, 5, 5).await;
        place(&pool, user, first, 6, 5).await.unwrap();
        let second_table = config::explore_layout::layout(102).unwrap().unwrap().areas.iter()
            .flat_map(|area| &area.units).find(|unit| unit.kind == 217 && unit.id != 29).unwrap().id;
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        let second = allocate_rock_uid(&mut tx, user, &map.state, second_table).await.unwrap();
        sqlx::query("INSERT INTO user_explore_runtime_items (uid, user_id, item_id, quantity, status) VALUES (?, ?, ?, 1, ?)")
            .bind(second).bind(user).bind(ROCK_ITEM).bind(PLACED_STATUS).execute(&mut *tx).await.unwrap();
        let table = map.state.objects.get_mut(&second_table).unwrap();
        table.status |= 4;
        set_table_rock(table, true);
        map.state.objects.insert(second as i32, ObjectState {
            kind: ROCK_KIND, status: 3, status2: String::new(), x: 1, y: 1, direction: 0,
        });
        explore_runtime::save_map(&mut tx, user, &mut map).await.unwrap();
        tx.commit().await.unwrap();
        set_position(&pool, user, 5, 12).await;
        take(&pool, user, 29).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap().state;
        assert_eq!(state.objects[&(first as i32)].status & 1, 0);
        assert_eq!(state.objects[&(second as i32)].status & 1, 1);
        assert_eq!(rock_source(second), Some((102, second_table)));
        assert_eq!(state.objects[&second_table].status2, r#"{"status":1}"#);
        let second_owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_explore_runtime_items WHERE user_id = ? AND uid = ?")
            .bind(user).bind(second).fetch_one(&mut *tx).await.unwrap();
        assert_eq!(second_owned, 1);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn placed_rock_can_be_recalled_from_its_table_across_maps() {
        let user = 992009;
        let pool = setup(user, 5, 12).await;
        let uid = take(&pool, user, 29).await.unwrap().item.uid.unwrap();
        let mut other = MapState::new(config::explore_layout::layout(101).unwrap().unwrap());
        other.x = 1; other.y = 6;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::insert_map(&mut tx, user, &other).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user, 101).await.unwrap();
        tx.commit().await.unwrap();
        // A new cross-map placement is rejected.
        assert!(place(&pool, user, uid, 1, 5).await.is_err());
        // Simulate a placement made by an older server before map isolation.
        let mut tx = pool.begin().await.unwrap();
        let mut other = explore_runtime::load_map(&mut tx, user, 101).await.unwrap().unwrap();
        other.state.objects.insert(uid as i32, ObjectState { kind: ROCK_KIND, status: 3,
            status2: String::new(), x: 1, y: 5, direction: 0 });
        explore_runtime::save_map(&mut tx, user, &mut other).await.unwrap();
        let mut origin = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        set_table_rock(origin.state.objects.get_mut(&29).unwrap(), true);
        explore_runtime::save_map(&mut tx, user, &mut origin).await.unwrap();
        sqlx::query("UPDATE user_explore_runtime_items SET status = -1 WHERE uid = ?")
            .bind(uid).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let origin = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        let status: serde_json::Value = serde_json::from_str(&origin.state.objects[&29].status2).unwrap();
        assert_eq!(status["status"], 1);
        assert!(status.get("rockUid").is_none());
        explore_runtime::set_current_map(&mut tx, user, 102).await.unwrap();
        tx.commit().await.unwrap();
        take(&pool, user, 29).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let origin = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        let other = explore_runtime::load_map(&mut tx, user, 101).await.unwrap().unwrap();
        assert_eq!(origin.state.objects[&29].status & 4, 0);
        assert_eq!(other.state.objects[&(uid as i32)].status & 1, 0);
        assert!(explore_runtime::list_items(&mut tx, user).await.unwrap().is_empty());
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn player_and_rock_can_hold_two_plates_without_losing_progress() {
        let user = 992002;
        let pool = setup(user, 8, 35).await;
        let mut tx = pool.begin().await.unwrap();
        let uid = explore_runtime::grant_item(&mut tx, user, ROCK_ITEM, 1).await.unwrap()[0].uid.unwrap();
        tx.commit().await.unwrap();
        place(&pool, user, uid, 8, 36).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        assert_eq!(map.state.objects[&96].status & 4, 4);
        assert_eq!(map.state.objects[&7].status & 4, 0);
        // Walk onto the second plate while the rock holds the first.
        let layout = config::explore_layout::layout(102).unwrap().unwrap();
        crate::explore_engine::move_basic(&mut map.state, layout, 9, 35).unwrap();
        crate::explore_engine::move_basic(&mut map.state, layout, 9, 36).unwrap();
        crate::explore_engine::move_basic(&mut map.state, layout, 10, 36).unwrap();
        assert_eq!(map.state.objects[&97].status & 4, 4);
        assert_eq!(map.state.objects[&7].status & 4, 4);
        explore_runtime::save_map(&mut tx, user, &mut map).await.unwrap();
        tx.commit().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let mut map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        assert_eq!(map.state.objects[&7].status & 4, 4);
        crate::explore_engine::move_basic(&mut map.state, layout, 9, 36).unwrap();
        assert_eq!(map.state.objects[&97].status & 4, 0);
        assert_eq!(map.state.objects[&7].status & 4, 0);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn disappearing_rune_chest_cannot_be_claimed_twice() {
        let user = 992003;
        let pool = setup(user, 20, 9).await;
        let claim = crate::explore_chest::claim(&pool, user, 36).await.unwrap();
        assert_eq!(claim.items[0].item_id, Some(150002));
        assert!(crate::explore_chest::claim(&pool, user, 36).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let map = explore_runtime::load_map(&mut tx, user, 102).await.unwrap().unwrap();
        assert_eq!(map.state.objects[&36].status & 1, 0);
        tx.commit().await.unwrap();
    }
}
