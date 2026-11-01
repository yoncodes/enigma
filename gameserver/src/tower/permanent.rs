//! Permanent tower victories are committed with their floor rewards and result.

use crate::{error::AppError, player::battle::ActiveBattle};
use database::{db::game::battle as battle_db, models::game::tower::TowerType};
use logic::reward::{self, AppliedRewards, RewardManager};
use sonettobuf::{EndDungeonPush, EpisodeNo, HeroNo, LayerNo, MaterialData, TowerBattleFinishPush};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub struct PermanentSettlement {
    pub finish: TowerBattleFinishPush,
    pub end_dungeon: EndDungeonPush,
    pub rewards: AppliedRewards,
}

pub fn is_permanent_battle(active: &ActiveBattle) -> bool {
    active.tower_type == Some(TowerType::Normal.id()) && active.tower_id.unwrap_or_default() == 0
}

/// Finish a normal permanent-tower fight exactly once. The fight marker,
/// per-episode state, floor advancement and first-clear rewards share a single
/// transaction; a failure leaves the active fight and all progress intact.
pub async fn settle_permanent_victory(
    db: &SqlitePool,
    player_id: i64,
    active: &ActiveBattle,
) -> Result<PermanentSettlement, AppError> {
    if !is_permanent_battle(active) || !active.is_victory() {
        return Err(AppError::InvalidRequest);
    }
    let fight_id = active.fight_id.ok_or(AppError::InvalidRequest)?;
    let layer_id = active.layer_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    let floor = tables.tower_permanent_episode.iter()
        .find(|row| row.layer_id == layer_id).ok_or(AppError::InvalidRequest)?;
    let episode_ids = floor.episode_ids.split('|')
        .map(|value| value.parse::<i32>().map_err(|_| AppError::InvalidRequest))
        .collect::<Result<Vec<_>, _>>()?;
    if episode_ids.is_empty() || !episode_ids.contains(&active.episode_id) {
        return Err(AppError::InvalidRequest);
    }
    let episode = tables.episode.get(active.episode_id).ok_or(AppError::InvalidRequest)?;
    if episode.chapter_id != active.chapter_id || episode.battle_id != active.battle_id {
        return Err(AppError::InvalidRequest);
    }
    let star = active.star()?;
    let mut tx = db.begin().await?;
    // This update guards duplicate and stale requests before any grant is made.
    battle_db::finish_fight_instance_in_transaction(&mut tx, player_id, fight_id).await?;
    sqlx::query("INSERT INTO user_towers (user_id, tower_type, tower_id) VALUES (?, 1, 0) \
        ON CONFLICT(user_id, tower_type, tower_id) DO NOTHING")
        .bind(player_id).execute(&mut *tx).await?;
    // For pre-existing saves, treat already passed layers as already rewarded.
    // Initialization is one-time so subsequently clearing floors out of order
    // does not hide the first-clear reward of an earlier floor.
    let initialized = sqlx::query("INSERT INTO user_tower_permanent_reward_init (user_id) VALUES (?) \
        ON CONFLICT(user_id) DO NOTHING").bind(player_id).execute(&mut *tx).await?.rows_affected() == 1;
    if initialized {
        let old_pass: i32 = sqlx::query_scalar("SELECT pass_layer_id FROM user_towers \
            WHERE user_id = ? AND tower_type = 1 AND tower_id = 0")
            .bind(player_id).fetch_one(&mut *tx).await?;
        for prior in tables.tower_permanent_episode.iter().filter(|row| row.layer_id <= old_pass) {
            sqlx::query("INSERT INTO user_tower_permanent_reward_claims (user_id, layer_id) VALUES (?, ?) \
                ON CONFLICT(user_id, layer_id) DO NOTHING")
                .bind(player_id).bind(prior.layer_id).execute(&mut *tx).await?;
        }
    }
    sqlx::query("INSERT INTO user_tower_layers (user_id, tower_type, tower_id, layer_id) \
        VALUES (?, 1, 0, ?) ON CONFLICT(user_id, tower_type, tower_id, layer_id) DO NOTHING")
        .bind(player_id).bind(layer_id).execute(&mut *tx).await?;
    for episode_id in &episode_ids {
        sqlx::query("INSERT INTO user_tower_episodes \
            (user_id, tower_type, tower_id, layer_id, episode_id) VALUES (?, 1, 0, ?, ?) \
            ON CONFLICT(user_id, tower_type, tower_id, layer_id, episode_id) DO NOTHING")
            .bind(player_id).bind(layer_id).bind(episode_id).execute(&mut *tx).await?;
    }
    let old_status: i32 = sqlx::query_scalar("SELECT status FROM user_tower_episodes \
        WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ? AND episode_id = ?")
        .bind(player_id).bind(layer_id).bind(active.episode_id).fetch_one(&mut *tx).await?;
    if !matches!(old_status, 0 | 1) { return Err(AppError::InvalidRequest); }
    if old_status != 1 {
        save_team(&mut tx, player_id, active, layer_id).await?;
        let boss_id = active.fight_group.as_ref().and_then(|group| group.assist_boss_id).unwrap_or(0);
        sqlx::query("UPDATE user_tower_episodes SET status = 1, assist_boss_id = ? \
            WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ? AND episode_id = ?")
            .bind(boss_id).bind(player_id).bind(layer_id).bind(active.episode_id)
            .execute(&mut *tx).await?;
    }
    let mut episode_n_os = Vec::with_capacity(episode_ids.len());
    for episode_id in &episode_ids {
        episode_n_os.push(load_episode(&mut tx, player_id, layer_id, *episode_id).await?);
    }
    let all_completed = episode_n_os.iter().all(|episode| episode.status == Some(1));
    let first_clear = if all_completed {
        sqlx::query("UPDATE user_towers SET pass_layer_id = MAX(pass_layer_id, ?) \
            WHERE user_id = ? AND tower_type = 1 AND tower_id = 0")
            .bind(layer_id).bind(player_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_tower_permanent_reward_claims (user_id, layer_id) VALUES (?, ?) \
            ON CONFLICT(user_id, layer_id) DO NOTHING")
            .bind(player_id).bind(layer_id).execute(&mut *tx).await?.rows_affected() == 1
    } else { false };
    let configured_rewards = if first_clear { reward::parse_strict(&floor.first_reward)? }
        else { reward::RewardSet::default() };
    let first_bonus = configured_rewards.material_changes().into_iter().map(|(kind, id, count)| MaterialData {
        materil_type: Some(kind), materil_id: Some(id), quantity: Some(count),
    }).collect();
    let rewards = RewardManager::new(player_id)
        .apply_dungeon_in_transaction(&mut tx, configured_rewards).await?;
    let history_high_score: i32 = sqlx::query_scalar("SELECT history_high_score FROM user_towers \
        WHERE user_id = ? AND tower_type = 1 AND tower_id = 0")
        .bind(player_id).fetch_one(&mut *tx).await?;
    let (curr_high_score, layer_high_score): (i32, i32) = sqlx::query_as(
        "SELECT curr_high_score, history_high_score FROM user_tower_layers \
         WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ?")
        .bind(player_id).bind(layer_id).fetch_one(&mut *tx).await?;
    let finish = TowerBattleFinishPush {
        r#type: Some(TowerType::Normal.id()), tower_id: Some(0), layer_id: Some(layer_id),
        difficulty: Some(active.difficulty.unwrap_or_default()), score: Some(0),
        boss_level: Some(active.assist_boss_level.unwrap_or_default()),
        team_level: Some(active.team_level.unwrap_or_default()),
        layer: Some(LayerNo { layer_id: Some(layer_id), curr_high_score: Some(curr_high_score),
            history_high_score: Some(layer_high_score), episode_n_os }),
        history_high_score: Some(history_high_score), params: active.params.clone(),
    };
    let end_dungeon = EndDungeonPush {
        chapter_id: Some(active.chapter_id), episode_id: Some(active.episode_id),
        player_exp: Some(0), first_bonus, star: Some(star), first_pass: Some(first_clear),
        update_dungeon_record: Some(false), can_update_dungeon_record: Some(false),
        old_record_round: Some(0), new_record_round: Some(0), extra_str: Some(String::new()),
        assist_user_id: Some(0), assist_nickname: Some(String::new()),
        total_round: Some(active.current_round()), ..Default::default()
    };
    tx.commit().await?;
    Ok(PermanentSettlement { finish, end_dungeon, rewards })
}

async fn save_team(tx: &mut Transaction<'_, Sqlite>, player: i64, active: &ActiveBattle, layer: i32)
    -> Result<(), AppError> {
    for table in ["user_tower_episode_hero_equips", "user_tower_episode_heroes"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE user_id = ? AND tower_type = 1 \
            AND tower_id = 0 AND layer_id = ? AND episode_id = ?"))
            .bind(player).bind(layer).bind(active.episode_id).execute(&mut **tx).await?;
    }
    let Some(group) = &active.fight_group else { return Ok(()) };
    for uid in group.hero_list.iter().chain(&group.sub_hero_list).copied().filter(|uid| *uid != 0) {
        let runtime_model = active.runtime.entity_info(uid).and_then(|entity| entity.model_id)
            .filter(|id| *id > 0);
        let model_id = if let Some(model_id) = runtime_model { model_id } else {
            sqlx::query_scalar::<_, i32>("SELECT hero_id FROM heroes WHERE uid = ? AND user_id = ?")
                .bind(uid).bind(player).fetch_optional(&mut **tx).await?.ok_or(AppError::InvalidRequest)?
        };
        let equips = group.equips.iter().find(|equip| equip.hero_uid == Some(uid))
            .map(|equip| equip.equip_uid.as_slice()).unwrap_or(&[]);
        insert_hero(tx, player, layer, active.episode_id, model_id, 0, equips).await?;
    }
    for trial in &group.trial_hero_list {
        let trial_id = trial.trial_id.filter(|id| *id > 0).ok_or(AppError::InvalidRequest)?;
        let hero_id = config::configs::get().hero_trial.get(trial_id)
            .ok_or(AppError::InvalidRequest)?.hero_id;
        insert_hero(tx, player, layer, active.episode_id, hero_id, trial_id, &trial.equip_uid).await?;
    }
    Ok(())
}

async fn insert_hero(tx: &mut Transaction<'_, Sqlite>, player: i64, layer: i32, episode: i32,
    hero_id: i32, trial_id: i32, equips: &[i64]) -> Result<(), AppError> {
    sqlx::query("INSERT INTO user_tower_episode_heroes \
        (user_id, tower_type, tower_id, layer_id, episode_id, hero_id, trial_id) VALUES (?, 1, 0, ?, ?, ?, ?)")
        .bind(player).bind(layer).bind(episode).bind(hero_id).bind(trial_id).execute(&mut **tx).await?;
    for uid in equips.iter().copied().filter(|uid| *uid > 0) {
        sqlx::query("INSERT OR IGNORE INTO user_tower_episode_hero_equips \
            (user_id, tower_type, tower_id, layer_id, episode_id, hero_id, equip_uid) VALUES (?, 1, 0, ?, ?, ?, ?)")
            .bind(player).bind(layer).bind(episode).bind(hero_id).bind(uid).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn load_episode(tx: &mut Transaction<'_, Sqlite>, player: i64, layer: i32, episode: i32)
    -> Result<EpisodeNo, AppError> {
    let (status, boss): (i32, i32) = sqlx::query_as("SELECT status, assist_boss_id FROM user_tower_episodes \
        WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ? AND episode_id = ?")
        .bind(player).bind(layer).bind(episode).fetch_one(&mut **tx).await?;
    let rows: Vec<(i32, i32)> = sqlx::query_as("SELECT hero_id, trial_id FROM user_tower_episode_heroes \
        WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ? AND episode_id = ? ORDER BY id")
        .bind(player).bind(layer).bind(episode).fetch_all(&mut **tx).await?;
    let mut heros = Vec::with_capacity(rows.len());
    for (hero_id, trial_id) in rows {
        let equip_uid = sqlx::query_scalar("SELECT equip_uid FROM user_tower_episode_hero_equips \
            WHERE user_id = ? AND tower_type = 1 AND tower_id = 0 AND layer_id = ? AND episode_id = ? AND hero_id = ? \
            ORDER BY equip_uid").bind(player).bind(layer).bind(episode).bind(hero_id).fetch_all(&mut **tx).await?;
        heros.push(HeroNo { hero_id: Some(hero_id), trial_id: Some(trial_id), equip_uid });
    }
    Ok(EpisodeNo { episode_id: Some(episode), status: Some(status), heros, assist_boss_id: Some(boss) })
}
