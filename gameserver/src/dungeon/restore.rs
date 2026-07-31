use crate::{error::AppError, player::battle::ActiveBattle};
use config::configs;
use database::db::game::battle;
use sonettobuf::StartDungeonRequest;
use sqlx::SqlitePool;

use super::{RefundSettlement, failure_refund, settle_checkpoint_refund};

pub enum ActiveFightRestore {
    Missing,
    Active(Box<ActiveBattle>),
    Refunded(Box<RefundSettlement>),
}

pub async fn restore_active_fight(
    pool: &SqlitePool,
    player_id: i64,
) -> Result<ActiveFightRestore, AppError> {
    let Some(record) = battle::load_active_fight(pool, player_id).await? else {
        return Ok(ActiveFightRestore::Missing);
    };

    let fight_id = record.id;
    let episode_id = record.episode_id;
    let multiplication = record.multiplication;
    let entry_cost = record.entry_cost.clone();
    match ActiveBattle::restore(pool, player_id, record).await {
        Ok(active) => Ok(ActiveFightRestore::Active(Box::new(active))),
        Err(AppError::InvalidBattleCheckpoint(error)) => {
            tracing::warn!(player_id, fight_id, %error, "refunding invalid fight checkpoint");
            let entry_cost = if entry_cost.is_empty() {
                let episode = configs::get()
                    .episode
                    .get(episode_id)
                    .ok_or(AppError::InvalidRequest)?;
                failure_refund(episode, multiplication)
            } else {
                serde_json::from_str(&entry_cost)
                    .map_err(|error| AppError::InvalidBattleCheckpoint(error.to_string()))?
            };
            Ok(ActiveFightRestore::Refunded(Box::new(
                settle_checkpoint_refund(pool, player_id, fight_id, entry_cost).await?,
            )))
        }
        Err(error) => Err(error),
    }
}

/// `is_restart` selects the client entry path; it does not change the saved dungeon inputs.
pub fn matches_saved_dungeon_start(active: &ActiveBattle, request: &StartDungeonRequest) -> bool {
    active.tower_context.is_none()
        && active.act229_context.is_none()
        && active.start_request.as_ref().is_some_and(|start| {
            start.chapter_id == request.chapter_id
                && start.episode_id == request.episode_id
                && start.fight_group == request.fight_group
                && start.multiplication == request.multiplication
                && start.use_record == request.use_record
                && start.is_balance == request.is_balance
                && start.params == request.params
        })
}
