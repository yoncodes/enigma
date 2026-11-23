use super::*;
use crate::{
    db::game::tower,
    models::game::tower::{TowerConstId, TowerType},
};
use common::time::ServerTime;
use sqlx::SqliteConnection;

const TOWER_OPEN: i32 = 2;

pub async fn load_tower_info(tx: &mut Transaction<'_, Sqlite>, user_id: i64) -> sqlx::Result<()> {
    let tables = configs::get();
    let now = ServerTime::now_ms();
    let mop_up_times: i32 = tables
        .tower_const
        .get(TowerConstId::MaxMopUpTimes.id())
        .and_then(|row| row.value.parse().ok())
        .unwrap_or_default();
    let trial_hero_season = tower::current_trial_hero_season(tables, now);

    sqlx::query(
        "INSERT INTO user_tower_info (user_id, mop_up_times, trial_hero_season) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(mop_up_times)
    .bind(trial_hero_season)
    .execute(&mut **tx)
    .await?;

    insert_tower(tx, user_id, TowerType::Normal, 0).await?;
    for boss in tables.tower_boss.iter() {
        insert_tower(tx, user_id, TowerType::Boss, boss.tower_id).await?;
    }

    for open in tower::tower_open_schedule(tables, now) {
        if open.tower_type == TowerType::Limited.id() && open.status == TOWER_OPEN {
            insert_tower(tx, user_id, TowerType::Limited, open.tower_id).await?;
        }
    }

    let custom_plan_count: i32 = tables
        .tower_const
        .get(TowerConstId::CustomTalentPlanCount.id())
        .and_then(|row| row.value.parse().ok())
        .unwrap_or(1);
    for boss in tables.tower_assist_boss.iter() {
        let level = tables
            .tower_assist_development(boss.boss_id)
            .map(|row| row.level)
            .min()
            .unwrap_or(1);
        let talent_point: i32 = tables
            .tower_assist_development(boss.boss_id)
            .filter(|row| row.level <= level)
            .map(|row| row.talent_point)
            .sum();
        let use_talent_plan = tables
            .tower_talent_plans(boss.boss_id)
            .map(|row| row.plan_id)
            .min()
            .unwrap_or(1);

        sqlx::query(
            "INSERT INTO user_assist_bosses (user_id, boss_id, level, use_talent_plan)
             VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(boss.boss_id)
        .bind(level)
        .bind(use_talent_plan)
        .execute(&mut **tx)
        .await?;
        for plan_id in 1..=custom_plan_count {
            sqlx::query(
                "INSERT INTO user_assist_boss_talent_plans
                    (user_id, boss_id, plan_id, talent_point, plan_name)
                 VALUES (?, ?, ?, ?, '')",
            )
            .bind(user_id)
            .bind(boss.boss_id)
            .bind(plan_id)
            .bind(talent_point)
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(())
}

async fn insert_tower(
    connection: &mut SqliteConnection,
    user_id: i64,
    tower_type: TowerType,
    tower_id: i32,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(tower_type.id())
        .bind(tower_id)
        .execute(connection)
        .await?;
    Ok(())
}
