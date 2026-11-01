//! Transactional exploration chest claims. Not yet wired to the network router.

use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
use database::db::game::explore_runtime;
use sonettobuf::{ExploreItemNo, ExploreStep};
use sqlx::SqlitePool;

use crate::{explore_engine, reward};

pub struct ChestClaim {
    pub chest_id: i32,
    pub items: Vec<ExploreItemNo>,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub steps: Vec<ExploreStep>,
}

/// Claim supported chests and configured area activations atomically.
/// A claim and all of its rewards are committed together, and repeats fail.
pub async fn claim(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<ChestClaim> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit))
    }).context("unknown exploration object")?;
    ensure!(matches!(unit.kind, 1 | 232), "object is not an exploration chest");
    ensure!(unit.done_effects.iter().all(|effect|
        (effect.0 == 21 && effect.1.as_str() == Some("0#1"))
            || (effect.0 == 1 && effect.1.as_str() == Some("")) || effect.0 == 11),
        "chest has unsupported done effects");
    let mut chest_id = None;
    let mut activations = BTreeSet::new();
    let mut reveals = BTreeSet::new();
    for effect in unit.done_effects.iter().filter(|effect| effect.0 == 11) {
        let target = explore_engine::counter_target(&effect.1).context("invalid chest area reveal")?;
        ensure!(layout.areas.iter().any(|area| area.id == target), "unknown chest area reveal");
        reveals.insert(target);
    }
    for effect in &unit.trigger_effects {
        match effect.0 {
            15 => {
                let id = effect.1.as_i64().and_then(|id| i32::try_from(id).ok())
                    .or_else(|| effect.1.as_str().and_then(|id| id.parse().ok()))
                    .context("invalid chest id")?;
                ensure!(chest_id.replace(id).is_none(), "chest has multiple rewards");
            }
            21 => {
                let (id, active) = effect.1.as_str().and_then(|value| value.split_once('#'))
                    .context("invalid chest activation")?;
                let id = id.parse::<i32>()?;
                ensure!(id > 0 && active == "1" && activations.insert(id), "unsupported chest activation");
            }
            // These effects are already performed by the client trigger sequence.
            20 | 31 | 33 => {}
            _ => anyhow::bail!("chest has unsupported trigger effects"),
        }
    }
    let chest_id = chest_id.context("chest reward is missing")?;
    let chest = config::explore_rewards::chest(chest_id)?.context("unknown chest reward")?;
    ensure!(matches!(chest.is_count, 0 | 1), "invalid chest counting flag");
    if chest.is_count == 1 {
        ensure!((chest.chapter_id, chest.episode_id) == (layout.chapter_id, layout.episode_id),
            "counted chest belongs to another map");
    }

    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "chest area is not open");
    let object = stored.state.objects.get(&object_id).context("chest state is missing")?;
    ensure!(object.kind == unit.kind && object.status & 3 == 3 && object.status & 12 == 0,
        "chest cannot be claimed");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "chest is too far away");
    let direction_matches = match unit.trigger_dir {
        0 => true, 1 => (dx, dy) == (-1, 0), 2 => (dx, dy) == (1, 0),
        3 => (dx, dy) == (0, 1), 4 => (dx, dy) == (0, -1), _ => false,
    };
    ensure!(direction_matches, "chest cannot be claimed from this direction");

    let mut exploration_items = Vec::new();
    let mut ordinary_parts = Vec::new();
    let mut materials = Vec::new();
    for part in chest.bonus.split('|') {
        let fields = part.split('#').map(str::parse::<i32>).collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(fields.len() == 3 && fields[1] > 0 && fields[2] > 0, "malformed chest reward");
        ensure!(matches!(fields[0], 1 | 2 | 9 | 15), "unsupported chest reward type");
        materials.push((fields[0] as u32, fields[1] as u32, fields[2]));
        if fields[0] == 15 {
            exploration_items.push((fields[1], fields[2]));
        } else {
            ordinary_parts.push(part);
        }
    }
    let ordinary_rewards = reward::parse_strict(&ordinary_parts.join("|"))?;
    stored.state.objects.get_mut(&object_id).context("chest state is missing")?.status |= 4;
    if unit.done_effects.iter().any(|effect| effect.0 == 1) {
        stored.state.objects.get_mut(&object_id).context("chest state is missing")?.status &= !1;
    }
    let mut changed = BTreeSet::from([object_id]);
    for id in activations {
        let target = layout.unit(id).context("unknown chest activation target")?;
        ensure!(target.kind == 6 && target.done_effects.is_empty()
            && target.trigger_effects.iter().all(|effect| matches!(effect.0, 11 | 22 | 33)),
            "unsupported chest activation target");
        let target_state = stored.state.objects.get_mut(&id).context("missing chest activation target")?;
        ensure!(target_state.kind == 6, "invalid chest activation state");
        if target_state.status & 4 == 0 { target_state.status |= 4; changed.insert(id); }
        let sources: Vec<_> = layout.areas.iter().flat_map(|area| &area.units)
            .filter(|source| source.trigger_effects.iter().any(|effect|
                effect.0 == 22 && explore_engine::counter_target(&effect.1) == Some(id))).collect();
        ensure!(!sources.is_empty(), "chest activation target has no counter sources");
        if sources.iter().all(|source| stored.state.objects.get(&source.id).is_some_and(|state| state.status & 4 != 0)) {
            for effect in target.trigger_effects.iter().filter(|effect| effect.0 == 11) {
                let area_id = explore_engine::counter_target(&effect.1).context("invalid counter area reveal")?;
                ensure!(layout.areas.iter().any(|area| area.id == area_id), "unknown counter area reveal");
                reveals.insert(area_id);
            }
        }
    }
    let mut shown_areas = BTreeSet::new();
    for area in reveals {
        if area != 0 && stored.state.areas.insert(area) { shown_areas.insert(area); }
    }
    if map_id == 301 { crate::explore_light::sync_light(&mut stored.state, layout, &mut changed)?; }
    let mut bonus_count = None;
    if chest.is_count == 1 {
        crate::explore_entry::ensure_map_summary(&mut tx, user_id, layout).await?;
        let inserted = sqlx::query("INSERT INTO user_explore_map_bonuses (user_id, map_id, bonus_id) \
            VALUES (?, ?, ?) ON CONFLICT(user_id, map_id, bonus_id) DO NOTHING")
            .bind(user_id).bind(map_id).bind(chest_id).execute(&mut *tx).await?;
        ensure!(inserted.rows_affected() == 1, "chest reward was already recorded");
        sqlx::query("UPDATE user_explore_maps SET bonus_num = bonus_num + 1 WHERE user_id = ? AND map_id = ?")
            .bind(user_id).bind(map_id).execute(&mut *tx).await?;
        bonus_count = Some(sqlx::query_scalar::<_, i32>(
            "SELECT bonus_num FROM user_explore_maps WHERE user_id = ? AND map_id = ?",
        ).bind(user_id).bind(map_id).fetch_one(&mut *tx).await?);
    }
    let mut items = Vec::new();
    for (item_id, amount) in exploration_items {
        items.extend(explore_runtime::grant_item(&mut tx, user_id, item_id, amount).await?);
    }
    let rewards = if ordinary_parts.is_empty() {
        reward::AppliedRewards::default()
    } else {
        reward::RewardManager::new(user_id).apply_in_transaction(&mut tx, pool, ordinary_rewards).await?
    };
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = explore_engine::changed_object_steps(&stored.state, &changed.into_iter().collect::<Vec<_>>())?;
    for area in shown_areas {
        steps.push(ExploreStep { param: Some(serde_json::json!({"stepType":14,"showAreas":[area]}).to_string()) });
    }
    if let Some(count) = bonus_count {
        steps.push(ExploreStep { param: Some(serde_json::json!({"stepType":5,"id":0,"num":count}).to_string()) });
    }
    let material_data: Vec<String> = materials.iter().map(|&(typ, id, quantity)| {
        serde_json::json!({"materilType":typ,"materilId":id,"quantity":quantity}).to_string()
    }).collect();
    steps.push(ExploreStep { param: Some(serde_json::json!({"stepType":4,"materialData":material_data}).to_string()) });
    tx.commit().await?;
    Ok(ChestClaim { chest_id, items, rewards,
        material_changes: materials.into_iter().filter(|entry| entry.0 != 15).collect(), steps })
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::{self, MapState};

    async fn setup(user_id: i64, x: i32, y: i32) -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind(format!("isolated-chest-{user_id}"))
            .execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        assert!(layout.node(x, y).is_some());
        let mut state = MapState::new(layout);
        state.x = x;
        state.y = y;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();
        pool
    }

    #[tokio::test]
    async fn item_chest_grants_one_persistent_exploration_item() {
        let user_id = 910001;
        let pool = setup(user_id, 28, 12).await;
        let granted = claim(&pool, user_id, 214).await.unwrap();
        assert_eq!(granted.chest_id, 1001);
        assert_eq!(granted.items.len(), 1);
        assert_eq!(granted.items[0].item_id, Some(150002));
        assert_eq!(granted.items[0].status, Some(0));
        assert!(claim(&pool, user_id, 214).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let items = explore_runtime::list_items(&mut tx, user_id).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uid, granted.items[0].uid);
        let state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!(state.state.objects[&214].status, 7);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn light_chapter_repair_chest_reveals_both_configured_areas() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 910003;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-light-chest").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(301).unwrap().unwrap();
        let mut map = MapState::new(layout);
        map.x = 28; map.y = 16;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
        tx.commit().await.unwrap();
        let result = claim(&pool, user_id, 221).await.unwrap();
        assert_eq!(result.chest_id, 2002);
        assert_eq!(result.items[0].item_id, Some(150004));
        let revealed: Vec<i64> = result.steps.iter().filter_map(|step| {
            let value: serde_json::Value = serde_json::from_str(step.param.as_ref()?).ok()?;
            (value["stepType"] == 14).then(|| value["showAreas"][0].as_i64()).flatten()
        }).collect();
        assert_eq!(revealed, vec![1001, 1002]);
        assert!(claim(&pool, user_id, 221).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
        assert!(saved.areas.contains(&1001) && saved.areas.contains(&1002));
        assert_eq!(saved.objects[&221].status & 1, 0);
        assert_eq!(saved.objects[&454].status & 4, 4);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn normal_chest_grants_configured_rewards_once() {
        crate::init_test_server_config();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(dir.to_str().unwrap()).expect("load isolated game configuration");
        let user_id = 910002;
        let pool = setup(user_id, 39, 4).await;
        let result = claim(&pool, user_id, 399).await.unwrap();
        assert_eq!(result.chest_id, 2);
        assert!(result.items.is_empty());
        let amount: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 2")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(amount, 30);
        assert!(claim(&pool, user_id, 399).await.is_err());
        let amount_after: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 2")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(amount_after, 30);
        let row: (i32, i32, i32, i32) = sqlx::query_as("SELECT bonus_num, bonus_num_total, gold_coin_total, purple_coin_total \
            FROM user_explore_maps WHERE user_id = ? AND map_id = 101")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(row, (1, 2, 2, 25));
    }
}
