//! Persist the client's ordered exploration bonus-scene choices.

use anyhow::{Context, Result, ensure};
use database::db::game::explore_runtime;
use sonettobuf::ExploreStep;
use sqlx::SqlitePool;

use crate::explore_engine::changed_object_steps;

fn validate_choices(scene: &config::explore_bonus::BonusScene, params: &str) -> Result<Vec<i32>> {
    let values = params.split('#').map(str::parse::<i32>).collect::<std::result::Result<Vec<_>, _>>()?;
    let mut cursor = 0;
    for (index, &count) in scene.choices_per_step.iter().enumerate() {
        if count > 0 {
            let value = *values.get(cursor).context("missing dialogue choice")?;
            ensure!(value > 0 && usize::try_from(value)? <= count, "dialogue choice is out of range");
            cursor += 1;
        }
        if index + 1 < scene.choices_per_step.len() {
            ensure!(values.get(cursor) == Some(&-1), "missing dialogue continuation marker");
            cursor += 1;
        }
    }
    ensure!(cursor == values.len(), "unexpected dialogue choices");
    Ok(values)
}

pub async fn finish(pool: &SqlitePool, user: i64, object_id: i32, params: &str) -> Result<Vec<ExploreStep>> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, unit) = layout.areas.iter().find_map(|area|
        area.units.iter().find(|unit| unit.id == object_id).map(|unit| (area, unit)))
        .context("unknown bonus scene")?;
    ensure!(unit.kind == 229 && unit.done_effects.is_empty() && unit.trigger_effects.len() == 1
        && unit.trigger_effects[0].0 == 1, "unsupported bonus scene effects");
    let disappear = crate::explore_engine::counter_target(&unit.trigger_effects[0].1).context("invalid scene disappearance")?;
    ensure!(disappear == 0 || disappear == object_id, "unsupported scene disappearance target");
    let scene_id = unit.special_data.first().and_then(crate::explore_engine::counter_target).context("missing bonus scene id")?;
    let scene = config::explore_bonus::scene(scene_id)?.context("unknown bonus scene configuration")?;
    let options = validate_choices(scene, params)?;
    let mut stored = explore_runtime::load_map(&mut tx, user, map_id).await?.context("missing map state")?;
    ensure!(area.id == 0 || stored.state.areas.contains(&area.id), "bonus scene area is closed");
    let object = stored.state.objects.get(&object_id).context("missing bonus scene state")?;
    ensure!(object.kind == 229 && object.status & 3 == 3 && object.status & 8 == 0, "bonus scene is unavailable");
    let dx = i64::from(stored.state.x) - i64::from(object.x);
    let dy = i64::from(stored.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "bonus scene is too far away");
    ensure!(match unit.trigger_dir {
        0 => true, 1 => (dx,dy) == (-1,0), 2 => (dx,dy) == (1,0),
        3 => (dx,dy) == (0,1), 4 => (dx,dy) == (0,-1), _ => false,
    }, "bonus scene cannot be used from this direction");
    sqlx::query("INSERT INTO user_explore_chapters (user_id, chapter_id) VALUES (?, ?) \
        ON CONFLICT(user_id, chapter_id) DO NOTHING")
        .bind(user).bind(layout.chapter_id).execute(&mut *tx).await?;
    let inserted = sqlx::query("INSERT INTO user_explore_bonus_scenes (user_id, chapter_id, bonus_scene_id) \
        VALUES (?, ?, ?) ON CONFLICT(user_id, chapter_id, bonus_scene_id) DO NOTHING")
        .bind(user).bind(layout.chapter_id).bind(scene_id).execute(&mut *tx).await?;
    ensure!(inserted.rows_affected() == 1, "bonus scene was already recorded");
    sqlx::query("INSERT INTO user_explore_runtime_bonus_options (user_id, chapter_id, bonus_scene_id, options_json) VALUES (?, ?, ?, ?)")
        .bind(user).bind(layout.chapter_id).bind(scene_id).bind(serde_json::to_string(&options)?)
        .execute(&mut *tx).await?;
    stored.state.objects.get_mut(&object_id).context("missing bonus scene state")?.status &= !1;
    explore_runtime::save_map(&mut tx, user, &mut stored).await?;
    let mut steps = changed_object_steps(&stored.state, &[object_id])?;
    steps.push(ExploreStep { param: Some(serde_json::json!({
        "stepType":17,"bonusSceneId":scene_id,"options":options,
    }).to_string()) });
    tx.commit().await?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn scene_choices_keep_order_and_roll_back_on_failure() {
        let cases = [
            (103, 710, 41, 94, 1401301, "-1#2#-1", vec![-1,2,-1]),
            (203, 621, -5, 86, 1403601, "-1#-1#3#-1#-1", vec![-1,-1,3,-1,-1]),
            (303, 839, 52, -6, 1402501, "-1#-1#1#-1#-1", vec![-1,-1,1,-1,-1]),
        ];
        for (map_id, object_id, x, y, scene_id, params, expected) in cases {
            let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
                .connect("sqlite::memory:").await.unwrap();
            database::run_migrations(&pool).await.unwrap();
            let user = 997000_i64 + i64::from(map_id);
            sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
                .bind(user).bind(format!("isolated-bonus-{map_id}")).execute(&pool).await.unwrap();
            let layout = config::explore_layout::layout(map_id).unwrap().unwrap();
            let mut map = MapState::new(layout);
            map.x = x; map.y = y;
            let unit_area = layout.node(x, y).unwrap().3;
            if unit_area != 0 { map.areas.insert(unit_area); }
            let mut tx = pool.begin().await.unwrap();
            explore_runtime::ensure_profile(&mut tx, user).await.unwrap();
            explore_runtime::insert_map(&mut tx, user, &map).await.unwrap();
            explore_runtime::set_current_map(&mut tx, user, map_id).await.unwrap();
            tx.commit().await.unwrap();
            assert!(finish(&pool, user, object_id, "99").await.is_err());
            sqlx::query("CREATE TRIGGER scene_rollback BEFORE INSERT ON user_explore_runtime_bonus_options \
                BEGIN SELECT RAISE(ABORT, 'simulated failure'); END")
                .execute(&pool).await.unwrap();
            assert!(finish(&pool, user, object_id, params).await.is_err());
            sqlx::query("DROP TRIGGER scene_rollback").execute(&pool).await.unwrap();
            let recorded: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_explore_bonus_scenes WHERE user_id = ?")
                .bind(user).fetch_one(&pool).await.unwrap();
            assert_eq!(recorded, 0);
            let steps = finish(&pool, user, object_id, params).await.unwrap();
            let step: serde_json::Value = serde_json::from_str(steps.last().unwrap().param.as_ref().unwrap()).unwrap();
            assert_eq!(step["bonusSceneId"].as_i64(), Some(i64::from(scene_id)));
            assert_eq!(step["options"], serde_json::json!(expected));
            assert!(finish(&pool, user, object_id, params).await.is_err());
            let (_, chapters, _, _) = database::db::game::explore::get_explore_info(&pool, user).await.unwrap();
            assert_eq!(chapters[0].bonus_scenes[0].bonus_scene_id, scene_id);
            assert_eq!(chapters[0].bonus_scenes[0].options, expected);
            let mut tx = pool.begin().await.unwrap();
            let saved = explore_runtime::load_map(&mut tx, user, map_id).await.unwrap().unwrap().state;
            assert_eq!(saved.objects[&object_id].status & 1, 0);
            tx.commit().await.unwrap();
        }
    }
}
