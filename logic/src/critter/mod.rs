use crate::error::AppError;
use common::types::critter_seat_operation::CritterSeatOperation;
use database::db::game::{
    activity_state::{self, ActivityStateKind, ActivityStateSet},
    critters, items,
};
use sonettobuf::{
    ChangeRestCritterReply, CritterBookInfo, CritterGetInfoReply, CritterRenameReply,
    GetCritterBookInfoReply, LockCritterReply, MarkCritterBookNewReadReply,
    SetCritterBookBackgroundReply, SetCritterBookUseSpecialSkinReply,
};
use sqlx::SqlitePool;

const CRITTER_BOOK_SCOPE_ID: i32 = 0;
const CRITTER_BOOK_BACKGROUND_SUB_TYPE: i32 = 33;

#[derive(Clone, Copy, Debug)]
pub struct CritterManager {
    player_id: i64,
}

impl CritterManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(&self, db: &SqlitePool) -> Result<CritterGetInfoReply, AppError> {
        critter_info(db, self.player_id).await
    }

    pub async fn rename(
        &self,
        db: &SqlitePool,
        critter_uid: i64,
        name: String,
    ) -> Result<CritterRenameReply, AppError> {
        critter_rename(db, self.player_id, critter_uid, name).await
    }

    pub async fn lock(
        &self,
        db: &SqlitePool,
        critter_uid: i64,
        lock: bool,
    ) -> Result<LockCritterReply, AppError> {
        lock_critter(db, self.player_id, critter_uid, lock).await
    }

    pub async fn change_rest(
        &self,
        db: &SqlitePool,
        building_uid: i64,
        operation: i32,
        slot_id1: i32,
        critter_uid: i64,
        slot_id2: i32,
    ) -> Result<ChangeRestCritterReply, AppError> {
        change_rest_critter(
            db,
            self.player_id,
            building_uid,
            operation,
            slot_id1,
            critter_uid,
            slot_id2,
        )
        .await
    }

    pub async fn book_info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetCritterBookInfoReply, AppError> {
        get_book_info(db, tables, self.player_id).await
    }

    pub async fn mark_book_read(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        id: i32,
    ) -> Result<MarkCritterBookNewReadReply, AppError> {
        mark_book_read(db, tables, self.player_id, id).await
    }

    pub async fn set_book_background(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        id: i32,
        background: i32,
    ) -> Result<SetCritterBookBackgroundReply, AppError> {
        set_book_background(db, tables, self.player_id, id, background).await
    }

    pub async fn set_book_special_skin(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        id: i32,
        use_special_skin: bool,
    ) -> Result<SetCritterBookUseSpecialSkinReply, AppError> {
        set_book_special_skin(db, tables, self.player_id, id, use_special_skin).await
    }
}

async fn critter_info(db: &SqlitePool, player_id: i64) -> Result<CritterGetInfoReply, AppError> {
    Ok(CritterGetInfoReply {
        critter_infos: critters::get_player_critters(db, player_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    })
}

async fn critter_rename(
    db: &SqlitePool,
    player_id: i64,
    critter_uid: i64,
    name: String,
) -> Result<CritterRenameReply, AppError> {
    critters::rename_critter(db, player_id, critter_uid, &name).await?;
    Ok(CritterRenameReply {
        uid: Some(critter_uid),
        name: Some(name),
    })
}

async fn lock_critter(
    db: &SqlitePool,
    player_id: i64,
    critter_uid: i64,
    lock: bool,
) -> Result<LockCritterReply, AppError> {
    critters::lock_critter(db, player_id, critter_uid, lock).await?;
    Ok(LockCritterReply {
        uid: Some(critter_uid),
        lock: Some(lock),
    })
}

async fn change_rest_critter(
    db: &SqlitePool,
    player_id: i64,
    building_uid: i64,
    operation: i32,
    slot_id1: i32,
    critter_uid: i64,
    slot_id2: i32,
) -> Result<ChangeRestCritterReply, AppError> {
    match CritterSeatOperation::from_id(operation).ok_or(AppError::InvalidRequest)? {
        CritterSeatOperation::Change => {
            critters::set_rest_slot(db, player_id, building_uid, slot_id1, critter_uid).await?;
        }
        CritterSeatOperation::Exchange => {
            critters::exchange_rest_slots(db, player_id, building_uid, slot_id1, slot_id2).await?;
        }
    }

    Ok(ChangeRestCritterReply {
        building_uid: Some(building_uid),
    })
}

async fn get_book_info(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
) -> Result<GetCritterBookInfoReply, AppError> {
    let states = activity_state::get(
        db,
        player_id,
        CRITTER_BOOK_SCOPE_ID,
        ActivityStateKind::CritterBook,
    )
    .await?;
    let book_infos = critters::get_owned_book_skins(db, player_id)
        .await?
        .into_iter()
        .filter(|(id, _)| tables.critter.get(*id).is_some())
        .map(|(id, unlock_special_skin)| {
            let (read, background, use_special_skin) = book_state(&states, id);
            CritterBookInfo {
                id: Some(id),
                unlock_special_skin: Some(unlock_special_skin),
                use_special_skin: Some(unlock_special_skin && use_special_skin),
                background: Some(background),
                unlock_normal_skin: Some(true),
                is_new: Some(!read),
            }
        })
        .collect();
    Ok(GetCritterBookInfoReply { book_infos })
}

async fn mark_book_read(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
    id: i32,
) -> Result<MarkCritterBookNewReadReply, AppError> {
    let (_, background, use_special_skin, _) = owned_book_state(db, tables, player_id, id).await?;
    save_book_state(db, player_id, id, true, background, use_special_skin).await?;
    Ok(MarkCritterBookNewReadReply { id: Some(id) })
}

async fn set_book_background(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
    id: i32,
    background: i32,
) -> Result<SetCritterBookBackgroundReply, AppError> {
    let (read, _, use_special_skin, _) = owned_book_state(db, tables, player_id, id).await?;
    if background != 0 {
        let configured = tables
            .item
            .get(background)
            .is_some_and(|item| item.sub_type == CRITTER_BOOK_BACKGROUND_SUB_TYPE);
        let owned = items::get_item(db, player_id, background as u32)
            .await?
            .is_some_and(|item| item.quantity > 0);
        if !configured || !owned {
            return Err(AppError::InvalidRequest);
        }
    }
    save_book_state(db, player_id, id, read, background, use_special_skin).await?;
    Ok(SetCritterBookBackgroundReply {
        id: Some(id),
        background: Some(background),
    })
}

async fn set_book_special_skin(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
    id: i32,
    use_special_skin: bool,
) -> Result<SetCritterBookUseSpecialSkinReply, AppError> {
    let (read, background, _, unlocked) = owned_book_state(db, tables, player_id, id).await?;
    if use_special_skin && !unlocked {
        return Err(AppError::InvalidRequest);
    }
    save_book_state(db, player_id, id, read, background, use_special_skin).await?;
    Ok(SetCritterBookUseSpecialSkinReply {
        id: Some(id),
        use_special_skin: Some(use_special_skin),
    })
}

async fn owned_book_state(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
    id: i32,
) -> Result<(bool, i32, bool, bool), AppError> {
    tables.critter.get(id).ok_or(AppError::InvalidRequest)?;
    let unlock_special_skin = critters::get_owned_book_skin(db, player_id, id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    let states = activity_state::get(
        db,
        player_id,
        CRITTER_BOOK_SCOPE_ID,
        ActivityStateKind::CritterBook,
    )
    .await?;
    let (read, background, use_special_skin) = book_state(&states, id);
    Ok((read, background, use_special_skin, unlock_special_skin))
}

fn book_state(states: &activity_state::ActivityStates, id: i32) -> (bool, i32, bool) {
    states
        .get(&id)
        .map(|(state, progress, ext)| (*state != 0, *progress, ext == "1"))
        .unwrap_or_default()
}

async fn save_book_state(
    db: &SqlitePool,
    player_id: i64,
    id: i32,
    read: bool,
    background: i32,
    use_special_skin: bool,
) -> Result<(), AppError> {
    activity_state::set(
        db,
        player_id,
        CRITTER_BOOK_SCOPE_ID,
        ActivityStateSet {
            kind: ActivityStateKind::CritterBook,
            entry_id: id,
            state: i32::from(read),
            progress: background,
            ext: if use_special_skin { "1" } else { "" },
        },
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
