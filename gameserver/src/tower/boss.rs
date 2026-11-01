//! Isle of Echoes boss victories: progress, first rewards, and client result.
use crate::{error::AppError, player::battle::ActiveBattle};
use database::{db::game::battle as battle_db, models::game::tower::TowerType};
use logic::reward::{self, RewardManager};
use sonettobuf::{EndDungeonPush, EpisodeNo, LayerNo, MaterialData, TowerBattleFinishPush};

use super::PermanentSettlement;

pub fn is_boss_battle(active: &ActiveBattle) -> bool {
    active.tower_type == Some(TowerType::Boss.id()) && active.layer_id.is_some()
}

pub async fn settle_boss_victory(db: &sqlx::SqlitePool, player_id: i64, active: &ActiveBattle) -> Result<PermanentSettlement, AppError> {
    if !is_boss_battle(active) || !active.is_victory() { return Err(AppError::InvalidRequest); }
    let fight_id = active.fight_id.ok_or(AppError::InvalidRequest)?;
    let tower_id = active.tower_id.ok_or(AppError::InvalidRequest)?;
    let layer_id = active.layer_id.ok_or(AppError::InvalidRequest)?;
    if layer_id == 0 { return settle_boss_teach(db, player_id, active, tower_id, fight_id).await; }
    if layer_id < 0 { return Err(AppError::InvalidRequest); }
    let tables = config::configs::get();
    let floor = tables.tower_boss_episode.iter().find(|row| row.tower_id == tower_id
        && row.layer_id == layer_id && row.episode_id == active.episode_id).ok_or(AppError::InvalidRequest)?;
    let boss = tables.tower_boss.iter().find(|row| row.tower_id == tower_id).ok_or(AppError::InvalidRequest)?;
    let episode = tables.episode.get(active.episode_id).ok_or(AppError::InvalidRequest)?;
    if episode.chapter_id != active.chapter_id || episode.battle_id != active.battle_id { return Err(AppError::InvalidRequest); }
    let star = active.star()?;
    let mut tx = db.begin().await?;
    let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE id = ? AND user_id = ? AND episode_id = ? AND battle_id = ? AND active = 1")
        .bind(fight_id).bind(player_id).bind(active.episode_id).bind(active.battle_id).fetch_one(&mut *tx).await?;
    if match_count != 1 { return Err(AppError::InvalidRequest); }
    battle_db::finish_fight_instance_in_transaction(&mut tx, player_id, fight_id).await?;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id) VALUES (?, 2, ?) ON CONFLICT(user_id, tower_type, tower_id) DO NOTHING")
        .bind(player_id).bind(tower_id).execute(&mut *tx).await?;
    let (old_pass, high_score): (i32, i32) = sqlx::query_as("SELECT pass_layer_id, history_high_score FROM user_towers WHERE user_id = ? AND tower_type = 2 AND tower_id = ?")
        .bind(player_id).bind(tower_id).fetch_one(&mut *tx).await?;
    if old_pass < floor.pre_layer_id { return Err(AppError::InvalidRequest); }
    let old_status: Option<i32> = sqlx::query_scalar("SELECT status FROM user_tower_episodes WHERE user_id = ? AND tower_type = 2 AND tower_id = ? AND layer_id = ? AND episode_id = ?")
        .bind(player_id).bind(tower_id).bind(layer_id).bind(active.episode_id).fetch_optional(&mut *tx).await?;
    if old_status.is_some_and(|value| !matches!(value, 0 | 1)) { return Err(AppError::InvalidRequest); }
    let first_clear = old_pass < layer_id && old_status.unwrap_or_default() == 0;
    sqlx::query("INSERT INTO user_tower_layers (user_id, tower_type, tower_id, layer_id) VALUES (?, 2, ?, ?) ON CONFLICT(user_id, tower_type, tower_id, layer_id) DO NOTHING")
        .bind(player_id).bind(tower_id).bind(layer_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO user_tower_episodes (user_id, tower_type, tower_id, layer_id, episode_id, status) VALUES (?, 2, ?, ?, ?, 1) \
        ON CONFLICT(user_id, tower_type, tower_id, layer_id, episode_id) DO UPDATE SET status = 1")
        .bind(player_id).bind(tower_id).bind(layer_id).bind(active.episode_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE user_towers SET pass_layer_id = MAX(pass_layer_id, ?) WHERE user_id = ? AND tower_type = 2 AND tower_id = ?")
        .bind(layer_id).bind(player_id).bind(tower_id).execute(&mut *tx).await?;

    let existing_level: Option<i32> = sqlx::query_scalar("SELECT level FROM user_assist_bosses WHERE user_id = ? AND boss_id = ?")
        .bind(player_id).bind(boss.boss_id).fetch_optional(&mut *tx).await?;
    let old_level = existing_level.unwrap_or(0).max(0);
    let new_level = old_level.max(floor.boss_level);
    let total_points: i32 = tables.tower_assist_development(boss.boss_id)
        .filter(|row| row.level <= new_level).map(|row| row.talent_point).sum();
    let added_points: i32 = tables.tower_assist_development(boss.boss_id)
        .filter(|row| row.level > old_level && row.level <= new_level).map(|row| row.talent_point).sum();
    let default_plan = tables.tower_talent_plans(boss.boss_id).map(|row| row.plan_id).min().unwrap_or(1);
    sqlx::query("INSERT INTO user_assist_bosses (user_id, boss_id, level, use_talent_plan) VALUES (?, ?, ?, ?) \
        ON CONFLICT(user_id, boss_id) DO UPDATE SET level = MAX(level, excluded.level)")
        .bind(player_id).bind(boss.boss_id).bind(new_level).bind(default_plan).execute(&mut *tx).await?;
    let plan_count: i32 = tables.tower_const.get(119).and_then(|row| row.value.parse().ok()).ok_or(AppError::InvalidRequest)?;
    if !(1..=20).contains(&plan_count) { return Err(AppError::InvalidRequest); }
    for plan in 1..=plan_count {
        let inserted = sqlx::query("INSERT INTO user_assist_boss_talent_plans (user_id, boss_id, plan_id, talent_point) VALUES (?, ?, ?, ?) ON CONFLICT(user_id, boss_id, plan_id) DO NOTHING")
            .bind(player_id).bind(boss.boss_id).bind(plan).bind(total_points).execute(&mut *tx).await?.rows_affected();
        if inserted == 0 && added_points > 0 {
            sqlx::query("UPDATE user_assist_boss_talent_plans SET talent_point = talent_point + ? WHERE user_id = ? AND boss_id = ? AND plan_id = ?")
                .bind(added_points).bind(player_id).bind(boss.boss_id).bind(plan).execute(&mut *tx).await?;
        }
    }
    let configured = if first_clear { reward::parse_strict(&floor.first_reward)? } else { reward::RewardSet::default() };
    let first_bonus = configured.material_changes().into_iter().map(|(kind,id,count)| MaterialData {
        materil_type: Some(kind), materil_id: Some(id), quantity: Some(count),
    }).collect();
    let rewards = RewardManager::new(player_id).apply_dungeon_in_transaction(&mut tx, configured).await?;
    let finish = TowerBattleFinishPush {
        r#type: Some(2), tower_id: Some(tower_id), layer_id: Some(layer_id), difficulty: Some(active.difficulty.unwrap_or_default()),
        score: Some(0), boss_level: Some(new_level), team_level: Some(active.team_level.unwrap_or_default()),
        layer: Some(LayerNo { layer_id: Some(layer_id), curr_high_score: Some(0), history_high_score: Some(0),
            episode_n_os: vec![EpisodeNo { episode_id: Some(active.episode_id), status: Some(1), heros: vec![], assist_boss_id: Some(0) }] }),
        history_high_score: Some(high_score), params: active.params.clone(),
    };
    let end_dungeon = EndDungeonPush { chapter_id: Some(active.chapter_id), episode_id: Some(active.episode_id),
        player_exp: Some(0), first_bonus, star: Some(star), first_pass: Some(first_clear),
        update_dungeon_record: Some(false), can_update_dungeon_record: Some(false), old_record_round: Some(0),
        new_record_round: Some(0), extra_str: Some(String::new()), assist_user_id: Some(0),
        assist_nickname: Some(String::new()), total_round: Some(active.current_round()), ..Default::default() };
    tx.commit().await?;
    Ok(PermanentSettlement { finish, end_dungeon, rewards })
}

async fn settle_boss_teach(db: &sqlx::SqlitePool, player_id: i64, active: &ActiveBattle,
    tower_id: i32, fight_id: i64) -> Result<PermanentSettlement, AppError> {
    let tables = config::configs::get();
    let teach_id = active.difficulty.ok_or(AppError::InvalidRequest)?;
    let teach = tables.tower_boss_teach.iter().find(|row| row.tower_id == tower_id && row.teach_id == teach_id
        && row.episode_id == active.episode_id).ok_or(AppError::InvalidRequest)?;
    let boss = tables.tower_boss.iter().find(|row| row.tower_id == tower_id).ok_or(AppError::InvalidRequest)?;
    let episode = tables.episode.get(active.episode_id).ok_or(AppError::InvalidRequest)?;
    if episode.chapter_id != active.chapter_id || episode.battle_id != active.battle_id { return Err(AppError::InvalidRequest); }
    let star = active.star()?;
    let mut tx = db.begin().await?;
    let matches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE id = ? AND user_id = ? AND episode_id = ? AND battle_id = ? AND active = 1")
        .bind(fight_id).bind(player_id).bind(active.episode_id).bind(active.battle_id).fetch_one(&mut *tx).await?;
    if matches != 1 { return Err(AppError::InvalidRequest); }
    battle_db::finish_fight_instance_in_transaction(&mut tx, player_id, fight_id).await?;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id) VALUES (?, 2, ?) ON CONFLICT(user_id, tower_type, tower_id) DO NOTHING")
        .bind(player_id).bind(tower_id).execute(&mut *tx).await?;
    let first_clear = sqlx::query("INSERT INTO user_tower_pass_teaches (user_id, tower_type, tower_id, teach_id) VALUES (?, 2, ?, ?) ON CONFLICT DO NOTHING")
        .bind(player_id).bind(tower_id).bind(teach_id).execute(&mut *tx).await?.rows_affected() == 1;
    let configured = if first_clear { reward::parse_strict(&teach.first_bonus)? } else { reward::RewardSet::default() };
    let first_bonus = configured.material_changes().into_iter().map(|(kind,id,count)| MaterialData {
        materil_type: Some(kind), materil_id: Some(id), quantity: Some(count),
    }).collect();
    let rewards = RewardManager::new(player_id).apply_dungeon_in_transaction(&mut tx, configured).await?;
    let boss_level: i32 = sqlx::query_scalar("SELECT COALESCE((SELECT level FROM user_assist_bosses WHERE user_id = ? AND boss_id = ?), 0)")
        .bind(player_id).bind(boss.boss_id).fetch_one(&mut *tx).await?;
    let finish = TowerBattleFinishPush { r#type: Some(2), tower_id: Some(tower_id), layer_id: Some(0),
        difficulty: Some(teach_id), score: Some(0), boss_level: Some(boss_level), team_level: Some(active.team_level.unwrap_or_default()),
        layer: None, history_high_score: Some(0), params: active.params.clone() };
    let end_dungeon = EndDungeonPush { chapter_id: Some(active.chapter_id), episode_id: Some(active.episode_id),
        player_exp: Some(0), first_bonus, star: Some(star), first_pass: Some(first_clear),
        update_dungeon_record: Some(false), can_update_dungeon_record: Some(false), old_record_round: Some(0),
        new_record_round: Some(0), extra_str: Some(String::new()), assist_user_id: Some(0),
        assist_nickname: Some(String::new()), total_round: Some(active.current_round()), ..Default::default() };
    tx.commit().await?;
    Ok(PermanentSettlement { finish, end_dungeon, rewards })
}
