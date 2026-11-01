//! Exploration battle context and settlement, awaiting network integration.

use anyhow::{Context, Result, ensure};
use database::db::game::{battle as battle_db, explore_runtime};
use sonettobuf::ExploreStep;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::explore_engine::changed_object_steps;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct PendingBattle {
    pub map_id: i32,
    pub unit_id: i32,
    pub battle_id: i32,
    pub fight_id: Option<i64>,
}

pub enum BattleInteraction {
    Begin { battle_id: i32, step: ExploreStep },
    Finish { steps: Vec<ExploreStep> },
}

async fn pending_in_transaction(tx: &mut Transaction<'_, Sqlite>, user_id: i64) -> Result<Option<PendingBattle>> {
    Ok(sqlx::query_as::<_, PendingBattle>(
        "SELECT map_id, unit_id, battle_id, fight_id FROM user_explore_pending_battles WHERE user_id = ?",
    ).bind(user_id).fetch_optional(&mut **tx).await?)
}

pub async fn pending(pool: &SqlitePool, user_id: i64) -> Result<Option<PendingBattle>> {
    let mut tx = pool.begin().await?;
    let pending = pending_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(pending)
}

fn status_info(status: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    if status.is_empty() {
        return Ok(serde_json::Map::new());
    }
    let value: serde_json::Value = serde_json::from_str(status)?;
    value.as_object().cloned().context("invalid battle interaction state")
}

/// The first interaction starts a configured battle. After a confirmed win,
/// the client's reload interaction finalizes that object's done effects.
pub async fn interact(pool: &SqlitePool, user_id: i64, object_id: i32) -> Result<BattleInteraction> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit))
    }).context("unknown battle object")?;
    ensure!(unit.kind == 3, "object is not an exploration battle");
    ensure!(unit.done_effects.iter().all(|effect| effect.0 == 1 && effect.1.as_str() == Some("")),
        "unsupported exploration battle completion effect");
    let mut battle_id = None;
    for effect in &unit.trigger_effects {
        match effect.0 {
            0 => {
                let id = effect.1.as_i64().and_then(|id| i32::try_from(id).ok())
                    .or_else(|| effect.1.as_str().and_then(|id| id.parse().ok()))
                    .context("invalid exploration battle id")?;
                ensure!(battle_id.replace(id).is_none(), "multiple exploration battles on one object");
            }
            31 | 33 => {} // The client handles dialogue and sound.
            _ => anyhow::bail!("unsupported exploration battle trigger"),
        }
    }
    let battle_id = battle_id.context("battle id is missing")?;
    ensure!(config::configs::get().battle.get(battle_id).is_some(), "unknown exploration battle");
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "battle area is not open");
    let object = stored.state.objects.get(&object_id).context("battle object state is missing")?;
    ensure!(object.kind == 3 && object.status & 3 == 3 && object.status & 8 == 0,
        "battle object cannot be used");
    let mut secondary = status_info(&object.status2)?;
    if secondary.get("success").and_then(|value| value.as_i64()) == Some(1) {
        secondary.remove("success");
        let object = stored.state.objects.get_mut(&object_id).context("battle object state is missing")?;
        object.status |= 8;
        // Defeated encounters must stop occupying their tile. Some client maps
        // omit an explicit Disappear effect even when the encounter blocks the
        // only path to a required exploration item.
        object.status &= !1;
        object.status2 = if secondary.is_empty() { String::new() } else { serde_json::to_string(&secondary)? };
        explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
        let steps = changed_object_steps(&stored.state, &[object_id])?;
        tx.commit().await?;
        return Ok(BattleInteraction::Finish { steps });
    }
    ensure!(object.status & 4 != 0, "battle object is not active");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "battle object is too far away");
    let direction_matches = match unit.trigger_dir {
        0 => true, 1 => (dx,dy) == (-1,0), 2 => (dx,dy) == (1,0),
        3 => (dx,dy) == (0,1), 4 => (dx,dy) == (0,-1), _ => false,
    };
    ensure!(direction_matches, "battle object cannot be used from this direction");
    if let Some(pending) = pending_in_transaction(&mut tx, user_id).await? {
        ensure!(pending.fight_id.is_none(), "an exploration fight is already active");
    }
    let inserted = sqlx::query("INSERT INTO user_explore_pending_battles (user_id, map_id, unit_id, battle_id) VALUES (?, ?, ?, ?) \
        ON CONFLICT(user_id) DO UPDATE SET map_id = excluded.map_id, unit_id = excluded.unit_id, \
        battle_id = excluded.battle_id WHERE user_explore_pending_battles.fight_id IS NULL")
        .bind(user_id).bind(map_id).bind(object_id).bind(battle_id).execute(&mut *tx).await?;
    ensure!(inserted.rows_affected() == 1, "exploration fight is already active");
    tx.commit().await?;
    Ok(BattleInteraction::Begin { battle_id, step: ExploreStep {
        param: Some(serde_json::json!({"stepType": 12, "battleId": battle_id}).to_string()),
    } })
}

/// Called in the same transaction that creates the actual fight instance.
pub async fn bind_fight(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, pending: &PendingBattle, fight_id: i64,
) -> Result<()> {
    let result = sqlx::query("UPDATE user_explore_pending_battles SET fight_id = ? \
        WHERE user_id = ? AND map_id = ? AND unit_id = ? AND battle_id = ? AND fight_id IS NULL")
        .bind(fight_id).bind(user_id).bind(pending.map_id).bind(pending.unit_id).bind(pending.battle_id)
        .execute(&mut **tx).await?;
    ensure!(result.rows_affected() == 1, "exploration battle changed before starting");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn encounter_uses_its_configured_battle_and_only_marks_it_after_victory() {
        crate::init_test_server_config();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(dir.to_str().unwrap()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 960001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-battle-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 29;
        state.y = 14;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();

        let BattleInteraction::Begin { battle_id, step } = interact(&pool, user_id, 431).await.unwrap() else { panic!("expected battle") };
        assert_eq!(battle_id, 501101);
        let value: serde_json::Value = serde_json::from_str(step.param.as_ref().unwrap()).unwrap();
        assert_eq!(value["battleId"], 501101);
        let context = pending(&pool, user_id).await.unwrap().unwrap();
        let mut tx = pool.begin().await.unwrap();
        let fight_id = battle_db::create_fight_instance_in_transaction(&mut tx, battle_db::NewFightInstance {
            user_id, episode_id: 140101, battle_id, multiplication: 1,
            entry_cost: "{}", checkpoint: "{}", created_at: 0,
        }).await.unwrap();
        bind_fight(&mut tx, user_id, &context, fight_id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(settle(&pool, user_id, fight_id, 501105, true).await.is_err());
        settle(&pool, user_id, fight_id, battle_id, true).await.unwrap();
        assert!(settle(&pool, user_id, fight_id, battle_id, true).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
        assert_eq!(status_info(&saved.objects[&431].status2).unwrap()["success"], 1);
        tx.commit().await.unwrap();
        let dungeon_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = ? AND episode_id = 140101")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(dungeon_rows, 0, "a single encounter must not clear the entire map");
        let BattleInteraction::Finish { steps } = interact(&pool, user_id, 431).await.unwrap() else { panic!("expected completion") };
        assert_eq!(steps.len(), 1);
        assert!(interact(&pool, user_id, 431).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
        assert_eq!(saved.objects[&431].status & 8, 8);
        assert_eq!(saved.objects[&431].status & 1, 0);
        let mut traversable = saved.clone();
        crate::explore_engine::move_client(&mut traversable, layout, 29, 15).unwrap();
        assert!(saved.objects[&431].status2.is_empty());
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let mut state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        state.state.x = 32;
        state.state.y = 2;
        explore_runtime::save_map(&mut tx, user_id, &mut state).await.unwrap();
        tx.commit().await.unwrap();
        let BattleInteraction::Begin { battle_id, .. } = interact(&pool, user_id, 428).await.unwrap() else { panic!("expected second battle") };
        assert_eq!(battle_id, 501105);
        let context = pending(&pool, user_id).await.unwrap().unwrap();
        let mut tx = pool.begin().await.unwrap();
        let failed_fight = battle_db::create_fight_instance_in_transaction(&mut tx, battle_db::NewFightInstance {
            user_id, episode_id: 140101, battle_id, multiplication: 1,
            entry_cost: "{}", checkpoint: "{}", created_at: 0,
        }).await.unwrap();
        bind_fight(&mut tx, user_id, &context, failed_fight).await.unwrap();
        tx.commit().await.unwrap();
        settle(&pool, user_id, failed_fight, battle_id, false).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
        assert_eq!(saved.objects[&428].status & 8, 0);
        assert!(saved.objects[&428].status2.is_empty());
        tx.commit().await.unwrap();
        assert!(matches!(interact(&pool, user_id, 428).await.unwrap(), BattleInteraction::Begin { .. }));
    }
}

/// Confirm victory or failure together with finishing the existing fight record.
pub async fn settle(pool: &SqlitePool, user_id: i64, fight_id: i64, battle_id: i32, won: bool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let pending = pending_in_transaction(&mut tx, user_id).await?.context("exploration battle is missing")?;
    ensure!(pending.fight_id == Some(fight_id) && pending.battle_id == battle_id,
        "exploration battle does not match the active fight");
    let layout = config::explore_layout::layout(pending.map_id)?.context("unknown exploration map")?;
    let matching_fight: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM fight_instances \
        WHERE id = ? AND user_id = ? AND episode_id = ? AND battle_id = ? AND active = 1)")
        .bind(fight_id).bind(user_id).bind(layout.episode_id).bind(battle_id).fetch_one(&mut *tx).await?;
    ensure!(matching_fight, "exploration fight is no longer active");
    if won {
        let mut stored = explore_runtime::load_map(&mut tx, user_id, pending.map_id).await?
            .context("exploration map state is missing")?;
        let object = stored.state.objects.get_mut(&pending.unit_id).context("battle object state is missing")?;
        ensure!(object.kind == 3 && object.status & 8 == 0, "battle object is already complete");
        let mut secondary = status_info(&object.status2)?;
        secondary.insert("success".into(), 1.into());
        object.status2 = serde_json::to_string(&secondary)?;
        explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    }
    battle_db::finish_fight_instance_in_transaction(&mut tx, user_id, fight_id).await?;
    sqlx::query("DELETE FROM user_explore_pending_battles WHERE user_id = ? AND fight_id = ?")
        .bind(user_id).bind(fight_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
