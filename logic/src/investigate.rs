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
mod tests {
    use super::*;

    async fn test_pool(user_id: i64) -> SqlitePool {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
        )
        .bind(user_id)
        .bind(format!("investigate-{user_id}"))
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn box_ids(info: &InvestigateInfo) -> Vec<(i32, Vec<i32>)> {
        info.intel_box
            .iter()
            .map(|group| (group.id.unwrap(), group.clue_ids.clone()))
            .collect()
    }

    #[tokio::test]
    async fn default_projection_has_only_default_clues_and_all_groups() {
        let pool = test_pool(5611).await;
        let info = InvestigateManager::new(5611)
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap();

        assert_eq!(info.clue_ids, vec![11, 41, 51, 61]);
        assert_eq!(
            box_ids(&info),
            vec![
                (1, vec![]),
                (2, vec![]),
                (3, vec![]),
                (4, vec![]),
                (5, vec![]),
                (6, vec![]),
            ]
        );
    }

    #[tokio::test]
    async fn present_dungeon_elements_unlock_all_configured_clues() {
        let pool = test_pool(5612).await;
        let mut expected = Vec::new();
        for (index, clue) in config::configs::get()
            .investigate_clue
            .iter()
            .filter(|clue| clue.map_element != 0)
            .enumerate()
        {
            expected.push(clue.id);
            sqlx::query(
                "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
                 VALUES (?, ?, ?)",
            )
            .bind(5612)
            .bind(clue.map_element)
            .bind(index != 0)
            .execute(&pool)
            .await
            .unwrap();
        }
        expected.extend(
            config::configs::get()
                .investigate_clue
                .iter()
                .filter(|clue| clue.default_unlock != 0)
                .map(|clue| clue.id),
        );
        expected.sort_unstable();

        let info = InvestigateManager::new(5612)
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap();

        assert_eq!(info.clue_ids, expected);
        assert_eq!(info.clue_ids.len(), 14);
        assert!(info.intel_box.iter().all(|group| group.clue_ids.is_empty()));
    }

    #[tokio::test]
    async fn put_clue_persists_a_valid_link_once() {
        let pool = test_pool(5613).await;
        let manager = InvestigateManager::new(5613);
        let expected = PutClueReply {
            id: Some(1),
            clue_id: Some(11),
        };

        assert_eq!(
            manager
                .put_clue(&pool, config::configs::get(), 1, 11)
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            manager
                .put_clue(&pool, config::configs::get(), 1, 11)
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_investigate_clues
                 WHERE user_id = 5613 AND info_id = 1 AND clue_id = 11",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        let info = InvestigateManager::new(5613)
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap();
        assert_eq!(
            info.intel_box
                .into_iter()
                .find(|group| group.id == Some(1))
                .unwrap()
                .clue_ids,
            vec![11]
        );
    }

    #[tokio::test]
    async fn put_clue_rejects_wrong_group_and_locked_clue() {
        let pool = test_pool(5614).await;
        let manager = InvestigateManager::new(5614);

        assert!(matches!(
            manager.put_clue(&pool, config::configs::get(), 2, 11).await,
            Err(AppError::InvalidRequest)
        ));
        assert!(matches!(
            manager.put_clue(&pool, config::configs::get(), 2, 21).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_investigate_clues WHERE user_id = 5614",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }
}
