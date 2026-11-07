use crate::error::AppError;
use database::{db::game::explore::state, models::game::explore::state::ItemState};
use sqlx::SqlitePool;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub struct UseItemOutcome {
    pub uid: i64,
    pub selected_uid: i64,
    pub deselected_item: Option<ItemState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemEffect {
    Active,
}

impl TryFrom<&str> for ItemEffect {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "2" => Ok(Self::Active),
            _ => Err(AppError::InvalidRequest),
        }
    }
}

pub(super) async fn use_item(
    db: &SqlitePool,
    player_id: i64,
    uid: i64,
    posx: i32,
    posy: i32,
    interact_id: Option<i32>,
) -> Result<UseItemOutcome, AppError> {
    if posx != 0 || posy != 0 || interact_id.is_some() {
        return Err(AppError::InvalidRequest);
    }
    let mut tx = db.begin().await?;
    let map_id = state::last_map(&mut tx, player_id)
        .await?
        .filter(|&id| id != 0)
        .ok_or(AppError::InvalidRequest)?;
    let mut saved = state::load(&mut tx, player_id, map_id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    if uid <= 0 || (saved.use_item_uid != 0 && saved.use_item_uid != uid) {
        return Err(AppError::InvalidRequest);
    }
    let mut matches = saved.items.iter().filter(|item| item.uid == uid);
    let item = matches.next().ok_or(AppError::InvalidRequest)?;
    if matches.next().is_some() || item.quantity <= 0 || item.status != 0 {
        return Err(AppError::InvalidRequest);
    }
    ItemEffect::try_from(
        config::get()
            .explore_item
            .get(item.item_id)
            .ok_or(AppError::InvalidRequest)?
            .effect
            .as_str(),
    )?;

    let selected_uid = if saved.use_item_uid == uid { 0 } else { uid };
    let deselected_item = (selected_uid == 0).then(|| item.clone());
    saved.use_item_uid = selected_uid;
    if !state::update(&mut tx, player_id, &saved).await? {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;
    Ok(UseItemOutcome {
        uid,
        selected_uid,
        deselected_item,
    })
}
