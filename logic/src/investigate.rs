use crate::error::AppError;
use config::GameDB;
use database::db::game::investigate;
use sonettobuf::{GetInvestigateReply, IntelBox, InvestigateInfo, PutClueReply};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Copy, Debug)]
pub struct InvestigateManager {
    player_id: i64,
}

impl InvestigateManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        &self,
        pool: &SqlitePool,
        tables: &GameDB,
    ) -> Result<GetInvestigateReply, AppError> {
        let linked = investigate::get_linked_clues(pool, self.player_id).await?;
        let elements = investigate::get_dungeon_element_ids(pool, self.player_id)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();

        let mut configured = HashSet::new();
        let mut group_ids = BTreeSet::new();
        let mut clue_ids = Vec::new();
        for clue in tables.investigate_clue.iter() {
            configured.insert((clue.info_id, clue.id));
            group_ids.insert(clue.info_id);
            if clue.default_unlock != 0 || elements.contains(&clue.map_element) {
                clue_ids.push(clue.id);
            }
        }

        let mut linked_by_group = BTreeMap::<i32, Vec<i32>>::new();
        for (info_id, clue_id) in linked {
            if configured.contains(&(info_id, clue_id)) {
                linked_by_group.entry(info_id).or_default().push(clue_id);
            }
        }

        clue_ids.sort_unstable();
        clue_ids.dedup();
        let intel_box = group_ids
            .into_iter()
            .map(|info_id| {
                let mut clue_ids = linked_by_group.remove(&info_id).unwrap_or_default();
                clue_ids.sort_unstable();
                clue_ids.dedup();
                IntelBox {
                    id: Some(info_id),
                    clue_ids,
                }
            })
            .collect();

        Ok(GetInvestigateReply {
            info: Some(InvestigateInfo {
                intel_box,
                clue_ids,
            }),
        })
    }

    pub async fn put_clue(
        &self,
        pool: &SqlitePool,
        tables: &GameDB,
        info_id: i32,
        clue_id: i32,
    ) -> Result<PutClueReply, AppError> {
        let clue = tables
            .investigate_clue
            .get(clue_id)
            .filter(|clue| clue.info_id == info_id)
            .ok_or(AppError::InvalidRequest)?;

        if clue.default_unlock == 0
            && !investigate::get_dungeon_element_ids(pool, self.player_id)
                .await?
                .contains(&clue.map_element)
        {
            return Err(AppError::InvalidRequest);
        }

        investigate::link_clue(pool, self.player_id, info_id, clue_id).await?;

        Ok(PutClueReply {
            id: Some(info_id),
            clue_id: Some(clue_id),
        })
    }
}

#[cfg(test)]
mod tests;
