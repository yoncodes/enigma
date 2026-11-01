//! Transactional storage for detailed exploration map state.
//!
//! Callers must keep profile creation, state changes, and any corresponding
//! inventory/reward updates in the same transaction.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use config::explore_layout::Layout;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};

const STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectState {
    pub kind: i32,
    pub status: i32,
    pub status2: String,
    pub x: i32,
    pub y: i32,
    pub direction: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapState {
    pub format_version: u32,
    pub map_id: i32,
    pub x: i32,
    pub y: i32,
    pub direction: i32,
    /// Last walked-over save-point marker; absent in older isolated snapshots.
    #[serde(default)]
    pub reset_anchor: Option<(i32, i32, i32)>,
    pub challenge_count: i32,
    pub areas: BTreeSet<i32>,
    pub visited_nodes: BTreeSet<(i32, i32)>,
    pub objects: BTreeMap<i32, ObjectState>,
}

impl MapState {
    pub fn new(layout: &Layout) -> Self {
        // Areas revealed by a map effect start hidden. Other configured areas,
        // including the ice-puzzle subareas, must be visible from the start.
        let gated_areas: BTreeSet<i32> = layout.areas.iter().flat_map(|area| &area.units)
            .flat_map(|unit| unit.trigger_effects.iter().chain(unit.done_effects.iter()))
            .filter(|effect| effect.0 == 11)
            .filter_map(|effect| effect.1.as_i64().and_then(|n| i32::try_from(n).ok())
                .or_else(|| effect.1.as_str().and_then(|s| s.parse().ok())))
            .collect();
        let mut areas: BTreeSet<i32> = layout.areas.iter().map(|area| area.id)
            .filter(|id| *id != 0 && !gated_areas.contains(id)).collect();
        if layout.spawn.area != 0 {
            areas.insert(layout.spawn.area);
        }
        let mut visited_nodes = BTreeSet::new();
        visited_nodes.insert((layout.spawn.x, layout.spawn.y));
        let mut objects: BTreeMap<i32, ObjectState> = layout.areas.iter().flat_map(|area| &area.units).map(|unit| {
            (unit.id, ObjectState {
                kind: unit.kind,
                status: unit.initial_status,
                status2: String::new(),
                x: unit.x,
                y: unit.y,
                direction: unit.direction,
            })
        }).collect();
        // Pipe entrances list the ID of the initially mounted pot in their
        // layout; the matching client reads the relationship from status2.
        for entrance in layout.areas.iter().flat_map(|area| &area.units).filter(|unit| unit.kind == 222) {
            if let Some(pot_id) = entrance.special_data.first().and_then(|value| value.as_str())
                .and_then(|value| value.parse::<i32>().ok()) {
                if let Some(object) = objects.get_mut(&entrance.id) {
                    object.status2 = format!("{{\"bindInteractId\":{pot_id}}}");
                }
                if let Some(pot) = objects.get_mut(&pot_id) {
                    pot.status2 = format!("{{\"bindInteractId\":{}}}", entrance.id);
                }
            }
        }
        Self {
            format_version: STATE_VERSION,
            map_id: layout.map_id,
            x: layout.spawn.x,
            y: layout.spawn.y,
            direction: layout.spawn.direction,
            reset_anchor: None,
            challenge_count: 0,
            areas,
            visited_nodes,
            objects,
        }
    }

    fn validate_header(&self, map_id: i32) -> Result<()> {
        ensure!(self.format_version == STATE_VERSION, "unsupported exploration state version");
        ensure!(self.map_id == map_id, "exploration state belongs to a different map");
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct StoredMap {
    pub revision: i64,
    pub state: MapState,
    map_id: i32,
}

/// Reject pre-existing summary-only data: it cannot be converted without the
/// missing positions and interaction states. Never replace it with defaults.
pub async fn ensure_profile(tx: &mut Transaction<'_, Sqlite>, user_id: i64) -> Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_explore_runtime_profiles WHERE user_id = ?)",
    ).bind(user_id).fetch_one(&mut **tx).await?;
    if exists {
        return Ok(());
    }

    let legacy: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM user_explore_info WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_chapters WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_chapter_archives WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_bonus_scenes WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_bonus_scene_options WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_maps WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_map_bonuses WHERE user_id = ?1
            UNION ALL SELECT 1 FROM user_explore_unlocked_maps WHERE user_id = ?1
        )",
    ).bind(user_id).fetch_one(&mut **tx).await?;
    ensure!(!legacy, "existing exploration progress has no compatible detailed state");

    sqlx::query("INSERT INTO user_explore_runtime_profiles (user_id) VALUES (?) ON CONFLICT(user_id) DO NOTHING")
        .bind(user_id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn load_map(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, map_id: i32,
) -> Result<Option<StoredMap>> {
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT revision, state_json FROM user_explore_runtime_maps WHERE user_id = ? AND map_id = ?",
    ).bind(user_id).bind(map_id).fetch_optional(&mut **tx).await?;
    row.map(|(revision, json)| {
        let state: MapState = serde_json::from_str(&json).context("invalid exploration state")?;
        state.validate_header(map_id)?;
        Ok(StoredMap { revision, state, map_id })
    }).transpose()
}

/// Insert an initial state only once. A concurrent or existing state is kept intact.
pub async fn insert_map(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, state: &MapState,
) -> Result<bool> {
    state.validate_header(state.map_id)?;
    let json = serde_json::to_string(state)?;
    let result = sqlx::query(
        "INSERT INTO user_explore_runtime_maps (user_id, map_id, state_json) VALUES (?, ?, ?) \
         ON CONFLICT(user_id, map_id) DO NOTHING",
    ).bind(user_id).bind(state.map_id).bind(json).execute(&mut **tx).await?;
    Ok(result.rows_affected() == 1)
}

/// Reject stale writes instead of overwriting another request's progress.
pub async fn save_map(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, stored: &mut StoredMap,
) -> Result<()> {
    stored.state.validate_header(stored.map_id)?;
    ensure!(stored.revision >= 0 && stored.revision < i64::MAX, "invalid exploration revision");
    let json = serde_json::to_string(&stored.state)?;
    let result = sqlx::query(
        "UPDATE user_explore_runtime_maps SET state_json = ?, revision = revision + 1 \
         WHERE user_id = ? AND map_id = ? AND revision = ?",
    ).bind(json).bind(user_id).bind(stored.map_id).bind(stored.revision)
        .execute(&mut **tx).await?;
    ensure!(result.rows_affected() == 1, "exploration state changed concurrently or is missing");
    stored.revision += 1;
    Ok(())
}

pub async fn set_current_map(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, map_id: i32,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE user_explore_runtime_profiles SET current_map_id = ?1 WHERE user_id = ?2 \
         AND EXISTS (SELECT 1 FROM user_explore_runtime_maps WHERE user_id = ?2 AND map_id = ?1)",
    ).bind(map_id).bind(user_id).execute(&mut **tx).await?;
    ensure!(result.rows_affected() == 1, "cannot select a missing exploration map");
    Ok(())
}

pub async fn current_map(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64,
) -> Result<Option<i32>> {
    Ok(sqlx::query_scalar::<_, Option<i32>>(
        "SELECT current_map_id FROM user_explore_runtime_profiles WHERE user_id = ?",
    ).bind(user_id).fetch_optional(&mut **tx).await?.flatten())
}

/// Grant non-stackable exploration items inside the caller's reward transaction.
pub async fn grant_item(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, item_id: i32, quantity: i32,
) -> Result<Vec<sonettobuf::ExploreItemNo>> {
    let item = config::explore_rewards::item(item_id)?.context("unknown exploration item")?;
    ensure!(!item.is_stackable, "stackable exploration items are not supported");
    ensure!((1..=100).contains(&quantity), "invalid exploration item quantity");
    let mut items = Vec::with_capacity(quantity as usize);
    for _ in 0..quantity {
        // Some item UIDs become dynamic map-object IDs. Keep them outside the
        // static layout's range and inside the protocol's signed 32-bit range.
        // sqlite_sequence also prevents ID reuse after an item is removed.
        let last_uid: Option<i64> = sqlx::query_scalar(
            "SELECT seq FROM sqlite_sequence WHERE name = 'user_explore_runtime_items'",
        ).fetch_optional(&mut **tx).await?;
        let uid = last_uid.unwrap_or(999_999).max(999_999) + 1;
        ensure!(uid <= i64::from(i32::MAX), "exploration item identifiers exhausted");
        let result = sqlx::query(
            "INSERT INTO user_explore_runtime_items (uid, user_id, item_id, quantity, status) VALUES (?, ?, ?, 1, 0)",
        ).bind(uid).bind(user_id).bind(item_id).execute(&mut **tx).await?;
        items.push(sonettobuf::ExploreItemNo {
            uid: Some(result.last_insert_rowid()),
            item_id: Some(item_id),
            quantity: Some(1),
            status: Some(0),
        });
    }
    Ok(items)
}

pub async fn list_items(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64,
) -> Result<Vec<sonettobuf::ExploreItemNo>> {
    let rows: Vec<(i64, i32, i32, i32)> = sqlx::query_as(
        // A placed rock remains owned in storage but is absent from the backpack.
        "SELECT uid, item_id, quantity, status FROM user_explore_runtime_items \
         WHERE user_id = ? AND NOT (item_id = 150001 AND status = -1) ORDER BY uid",
    ).bind(user_id).fetch_all(&mut **tx).await?;
    Ok(rows.into_iter().map(|(uid, item_id, quantity, status)| sonettobuf::ExploreItemNo {
        uid: Some(uid), item_id: Some(item_id), quantity: Some(quantity), status: Some(status),
    }).collect())
}

pub async fn use_item_uid(tx: &mut Transaction<'_, Sqlite>, user_id: i64) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT use_item_uid FROM user_explore_runtime_profiles WHERE user_id = ?",
    ).bind(user_id).fetch_optional(&mut **tx).await?)
}

pub async fn select_item(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, uid: i64,
) -> Result<()> {
    let result = sqlx::query("UPDATE user_explore_runtime_profiles SET use_item_uid = ? \
        WHERE user_id = ? AND (? = 0 OR EXISTS(SELECT 1 FROM user_explore_runtime_items \
        WHERE user_id = ? AND uid = ? AND status >= 0))")
        .bind(uid).bind(user_id).bind(uid).bind(user_id).bind(uid)
        .execute(&mut **tx).await?;
    ensure!(result.rows_affected() == 1, "exploration item is not owned by this user");
    Ok(())
}

pub async fn update_item_status(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, uid: i64, previous: i32, next: i32,
) -> Result<sonettobuf::ExploreItemNo> {
    let changed = sqlx::query("UPDATE user_explore_runtime_items SET status = ? \
        WHERE user_id = ? AND uid = ? AND status = ?")
        .bind(next).bind(user_id).bind(uid).bind(previous).execute(&mut **tx).await?;
    ensure!(changed.rows_affected() == 1, "exploration item changed concurrently or is missing");
    let (item_id, quantity, status): (i32, i32, i32) = sqlx::query_as(
        "SELECT item_id, quantity, status FROM user_explore_runtime_items WHERE user_id = ? AND uid = ?",
    ).bind(user_id).bind(uid).fetch_one(&mut **tx).await?;
    Ok(sonettobuf::ExploreItemNo {
        uid: Some(uid), item_id: Some(item_id), quantity: Some(quantity), status: Some(status),
    })
}
