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

/// Take a rock from one configured stone table; the table cannot grant twice.
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
    ensure!(object.kind == 217 && object.status & 3 == 3 && object.status & 12 == 0,
        "stone table is unavailable");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "stone table is too far away");
    ensure!(match unit.trigger_dir {
        0 => true, 1 => (dx, dy) == (-1, 0), 2 => (dx, dy) == (1, 0),
        3 => (dx, dy) == (0, 1), 4 => (dx, dy) == (0, -1), _ => false,
    }, "stone table cannot be used from this direction");
    stored.state.objects.get_mut(&object_id).context("missing stone table state")?.status |= 4;
    let item = explore_runtime::grant_item(&mut tx, user_id, ROCK_ITEM, 1).await?
        .into_iter().next().context("failed to grant rock")?;
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &[object_id])?;
    steps.push(ExploreStep { param: Some(serde_json::json!({"stepType":4,"materialData":[
        serde_json::json!({"materilType":15,"materilId":ROCK_ITEM,"quantity":1}).to_string()
    ]}).to_string()) });
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
