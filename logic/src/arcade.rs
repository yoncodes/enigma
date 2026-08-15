use crate::{error::AppError, reward};
use config::GameDB;
use database::db::game::arcade;
use sonettobuf::{
    ArcadeAttrContainer, ArcadeAttrValue, ArcadeBoardInfo, ArcadeBook, ArcadeBookInfo,
    ArcadeGainRewardReply, ArcadeGetOutSideInfoReply, ArcadeOutSideInfo, ArcadeOutSideProp,
    ArcadePlayer, ArcadePlayerMoveReply, ArcadePos, ArcadeSwitchCharacterReply, ArcadeTalent,
    ArcadeTalentInfo, ArcadeTalentUpgradeReply, ArcadeUnitInfo,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

const ACTIVITY_TYPE_ID: i32 = 222;
const DEFAULT_CHARACTER_CONST_ID: i32 = 1;
const DEFAULT_POSITION_CONST_ID: i32 = 3;
const HALL_SIZE_CONST_ID: i32 = 4;
const DIAMOND_ATTR_ID: i32 = 202;

#[derive(Clone, Copy, Debug)]
pub struct ArcadeOutsideManager {
    player_id: i64,
}

pub struct TalentUpgrade {
    pub reply: ArcadeTalentUpgradeReply,
    pub changed_attr: ArcadeAttrValue,
}

pub struct RewardClaim {
    pub reply: ArcadeGainRewardReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub red_dot_id: i32,
}

impl ArcadeOutsideManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
    ) -> Result<ArcadeGetOutSideInfoReply, AppError> {
        let activity_id = self.ensure_state(db, tables).await?;
        Ok(ArcadeGetOutSideInfoReply {
            info: Some(self.snapshot(db, activity_id).await?),
        })
    }

    pub async fn move_player(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        x: i32,
        y: i32,
    ) -> Result<ArcadePlayerMoveReply, AppError> {
        let activity_id = self.ensure_state(db, tables).await?;
        let (width, height) = parse_pair(self.const_value(tables, HALL_SIZE_CONST_ID)?)?;
        if x < 1 || x > width || y < 1 || y > height {
            return Err(AppError::InvalidRequest);
        }
        arcade::move_player(
            db,
            self.player_id,
            activity_id,
            x,
            y,
            common::time::ServerTime::now_ms(),
        )
        .await?;
        Ok(ArcadePlayerMoveReply {
            x: Some(x),
            y: Some(y),
        })
    }

    pub async fn switch_character(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        character_id: i32,
    ) -> Result<ArcadeSwitchCharacterReply, AppError> {
        let activity_id = self.ensure_state(db, tables).await?;
        if tables.arcade_character.get(character_id).is_none()
            || !arcade::switch_character(
                db,
                self.player_id,
                activity_id,
                character_id,
                common::time::ServerTime::now_ms(),
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(ArcadeSwitchCharacterReply {
            character_id: Some(character_id),
        })
    }

    pub async fn upgrade_talent(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        talent_id: i32,
        expected_level: i32,
    ) -> Result<TalentUpgrade, AppError> {
        if expected_level < 0 {
            return Err(AppError::InvalidRequest);
        }
        let activity_id = self.ensure_state(db, tables).await?;
        let next_level = expected_level
            .checked_add(1)
            .ok_or(AppError::InvalidRequest)?;
        let row = tables
            .arcade_talent
            .iter()
            .find(|row| row.id == talent_id && row.level == next_level)
            .ok_or(AppError::InvalidRequest)?;
        let diamond_attr = tables
            .arcade_attribute
            .get(DIAMOND_ATTR_ID)
            .ok_or(AppError::InvalidRequest)?;
        if row.cost < 0 {
            return Err(AppError::InvalidRequest);
        }

        let mut tx = db.begin().await?;
        let attr = arcade::upgrade_talent_in_transaction(
            &mut tx,
            self.player_id,
            activity_id,
            arcade::TalentUpgradeCost {
                talent_id,
                expected_level,
                attr_id: DIAMOND_ATTR_ID,
                amount: row.cost,
                attr_min: diamond_attr.min,
            },
        )
        .await?
        .ok_or(AppError::InvalidRequest)?;
        tx.commit().await?;

        Ok(TalentUpgrade {
            reply: ArcadeTalentUpgradeReply {
                talent_id: Some(talent_id),
                level: Some(next_level),
            },
            changed_attr: attr_message(attr),
        })
    }

    pub async fn gain_rewards(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        request_reward_id: i32,
    ) -> Result<RewardClaim, AppError> {
        if request_reward_id != 0 {
            return Err(AppError::InvalidRequest);
        }
        let activity_id = self.ensure_state(db, tables).await?;
        let state = arcade::get_state(db, self.player_id, activity_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let red_dot_root = tables
            .activity
            .get(activity_id)
            .map(|row| row.red_dot_id)
            .filter(|id| *id != 0)
            .ok_or(AppError::InvalidRequest)?;
        let red_dot_id = tables
            .reddot
            .iter()
            .find(|row| {
                row.is_online != 0
                    && row
                        .parent
                        .split('#')
                        .any(|parent| parent.parse::<i32>().ok() == Some(red_dot_root))
            })
            .map(|row| row.id)
            .ok_or(AppError::InvalidRequest)?;
        let claimed = arcade::get_claims(db, self.player_id, activity_id).await?;
        let candidates = tables
            .arcade_reward
            .iter()
            .filter(|row| {
                row.activity_id == activity_id
                    && row.score <= state.score
                    && !claimed.contains(&row.id)
            })
            .collect::<Vec<_>>();

        let mut tx = db.begin().await?;
        let mut reward_set = reward::RewardSet::default();
        for row in candidates {
            if tables.reward(row.reward).is_none() {
                return Err(AppError::InvalidRequest);
            }
            if arcade::claim_reward_in_transaction(
                &mut tx,
                self.player_id,
                activity_id,
                row.id,
                common::time::ServerTime::now_ms(),
            )
            .await?
            {
                reward_set.extend(reward::parse_reward_id(row.reward));
            }
        }
        let material_changes = reward_set.material_changes();
        let rewards = reward::RewardManager::new(self.player_id)
            .apply_in_transaction(&mut tx, db, reward_set)
            .await?;
        tx.commit().await?;
        Ok(RewardClaim {
            reply: ArcadeGainRewardReply {
                reward_id: Some(request_reward_id),
            },
            rewards,
            material_changes,
            red_dot_id,
        })
    }

    async fn ensure_state(&self, db: &SqlitePool, tables: &GameDB) -> Result<i32, AppError> {
        let activity_id = tables
            .latest_open_activity_id(ACTIVITY_TYPE_ID)
            .ok_or(AppError::InvalidRequest)?;
        if arcade::get_state(db, self.player_id, activity_id)
            .await?
            .is_some()
        {
            return Ok(activity_id);
        }
        let character_id = self
            .const_value(tables, DEFAULT_CHARACTER_CONST_ID)?
            .parse()
            .map_err(|_| AppError::InvalidRequest)?;
        let character = tables
            .arcade_character
            .get(character_id)
            .ok_or(AppError::InvalidRequest)?;
        let (x, y) = parse_pair(self.const_value(tables, DEFAULT_POSITION_CONST_ID)?)?;
        let difficulty_id = tables
            .arcade_difficulty
            .iter()
            .filter(|row| row.level > 0)
            .map(|row| row.level)
            .min()
            .ok_or(AppError::InvalidRequest)?;
        let mut tx = db.begin().await?;
        arcade::create_state(
            &mut tx,
            self.player_id,
            arcade::NewOutsideState {
                activity_id,
                character_id,
                x,
                y,
            },
            difficulty_id,
            character.collection,
            common::time::ServerTime::now_ms(),
        )
        .await?;
        tx.commit().await?;
        Ok(activity_id)
    }

    async fn snapshot(
        &self,
        db: &SqlitePool,
        activity_id: i32,
    ) -> Result<ArcadeOutSideInfo, AppError> {
        let state = arcade::get_state(db, self.player_id, activity_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let talents = arcade::get_talents(db, self.player_id, activity_id).await?;
        let attrs = arcade::get_attrs(db, self.player_id, activity_id).await?;
        let books = arcade::get_books(db, self.player_id, activity_id).await?;
        let claims = arcade::get_claims(db, self.player_id, activity_id).await?;
        let mut grouped = BTreeMap::<i32, (Vec<i32>, Vec<i32>)>::new();
        for (book_type, element_id, is_new) in books {
            let entry = grouped.entry(book_type).or_default();
            entry.0.push(element_id);
            if is_new {
                entry.1.push(element_id);
            }
        }
        let hotfix = serde_json::from_str(&state.hotfix)?;
        Ok(ArcadeOutSideInfo {
            activity_id: None,
            board_info: Some(ArcadeBoardInfo { cells: Vec::new() }),
            talent_info: Some(ArcadeTalentInfo {
                talents: talents
                    .into_iter()
                    .map(|(id, level)| ArcadeTalent {
                        id: Some(id),
                        level: Some(level),
                    })
                    .collect(),
            }),
            book_info: Some(ArcadeBookInfo {
                books: [4, 1, 2, 3]
                    .into_iter()
                    .map(|book_type| {
                        let (ele_id, new_ele_id) = grouped.remove(&book_type).unwrap_or_default();
                        ArcadeBook {
                            r#type: Some(book_type),
                            ele_id,
                            new_ele_id,
                        }
                    })
                    .collect(),
            }),
            unit_info: Some(ArcadeUnitInfo { units: Vec::new() }),
            player: Some(ArcadePlayer {
                id: Some(state.character_id),
                pos: Some(ArcadePos {
                    x: Some(state.x),
                    y: Some(state.y),
                }),
                dir: Some(state.dir),
                skill_counter_box: Vec::new(),
                attack_attr_id: None,
            }),
            attr_container: Some(ArcadeAttrContainer {
                attr_values: attrs.into_iter().map(attr_message).collect(),
            }),
            prop: Some(ArcadeOutSideProp {
                hotfix,
                score: Some(state.score),
                gain: claims,
                unlock_role_ids: arcade::get_unlock_roles(db, self.player_id, activity_id).await?,
                unlock_difficulty_ids: arcade::get_unlock_difficulties(
                    db,
                    self.player_id,
                    activity_id,
                )
                .await?,
                return_reward_data: None,
            }),
        })
    }

    fn const_value<'a>(&self, tables: &'a GameDB, id: i32) -> Result<&'a str, AppError> {
        tables
            .arcade_const
            .get(id)
            .map(|row| row.value.as_str())
            .ok_or(AppError::InvalidRequest)
    }
}

fn parse_pair(value: &str) -> Result<(i32, i32), AppError> {
    let (left, right) = value.split_once('#').ok_or(AppError::InvalidRequest)?;
    Ok((
        left.parse().map_err(|_| AppError::InvalidRequest)?,
        right.parse().map_err(|_| AppError::InvalidRequest)?,
    ))
}

fn attr_message(attr: arcade::AttrState) -> ArcadeAttrValue {
    ArcadeAttrValue {
        id: Some(attr.attr_id),
        base: Some(attr.base),
        rate: Some(attr.rate),
        extra: (attr.extra != 0).then_some(attr.extra),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool(player_id: i64) -> SqlitePool {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(player_id)
            .bind(format!("arcade-{player_id}"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn fresh_snapshot_matches_captured_presence_and_config_defaults() {
        let pool = test_pool(5651).await;
        let info = ArcadeOutsideManager::new(5651)
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap();

        assert_eq!(info.activity_id, None);
        assert_eq!(info.board_info.unwrap().cells, vec![]);
        assert_eq!(info.talent_info.unwrap().talents, vec![]);
        assert_eq!(info.unit_info.unwrap().units, vec![]);
        assert_eq!(info.attr_container.unwrap().attr_values, vec![]);
        let books = info.book_info.unwrap().books;
        assert_eq!(
            books
                .iter()
                .map(|book| book.r#type.unwrap())
                .collect::<Vec<_>>(),
            vec![4, 1, 2, 3]
        );
        assert_eq!(books[1].ele_id, vec![101]);
        assert_eq!(books[1].new_ele_id, vec![101]);
        assert_eq!(books[2].ele_id, vec![16005]);
        assert_eq!(books[2].new_ele_id, vec![16005]);
        let player = info.player.unwrap();
        assert_eq!(player.id, Some(101));
        assert_eq!(
            player.pos,
            Some(ArcadePos {
                x: Some(6),
                y: Some(4)
            })
        );
        assert_eq!(player.dir, Some(0));
        assert_eq!(info.prop.unwrap().unlock_difficulty_ids, vec![1]);
    }

    #[tokio::test]
    async fn movement_is_bounded_and_persistent_and_locked_switch_is_rejected() {
        let pool = test_pool(5652).await;
        let manager = ArcadeOutsideManager::new(5652);
        manager.info(&pool, config::configs::get()).await.unwrap();
        assert!(matches!(
            manager
                .move_player(&pool, config::configs::get(), 0, 4)
                .await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            manager
                .move_player(&pool, config::configs::get(), 12, 12)
                .await
                .unwrap(),
            ArcadePlayerMoveReply {
                x: Some(12),
                y: Some(12)
            }
        );
        let player = manager
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap()
            .player
            .unwrap();
        assert_eq!(
            player.pos,
            Some(ArcadePos {
                x: Some(12),
                y: Some(12)
            })
        );
        assert!(matches!(
            manager
                .switch_character(&pool, config::configs::get(), 102)
                .await,
            Err(AppError::InvalidRequest)
        ));
    }

    #[tokio::test]
    async fn talent_upgrade_uses_current_level_and_atomically_spends_diamonds() {
        let pool = test_pool(5653).await;
        let manager = ArcadeOutsideManager::new(5653);
        manager.info(&pool, config::configs::get()).await.unwrap();
        let activity_id = config::configs::get()
            .latest_open_activity_id(ACTIVITY_TYPE_ID)
            .unwrap();
        sqlx::query("INSERT INTO user_arcade_attrs (user_id, activity_id, attr_id, base, rate, extra) VALUES (5653, ?, 202, 240, 0, 0)")
            .bind(activity_id).execute(&pool).await.unwrap();

        let upgrade = manager
            .upgrade_talent(&pool, config::configs::get(), 100, 0)
            .await
            .unwrap();
        assert_eq!(upgrade.reply.level, Some(1));
        assert_eq!(
            upgrade.changed_attr,
            ArcadeAttrValue {
                id: Some(202),
                base: Some(190),
                rate: Some(0),
                extra: None
            }
        );
        assert!(matches!(
            manager
                .upgrade_talent(&pool, config::configs::get(), 100, 0)
                .await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(sqlx::query_scalar::<_, i32>("SELECT base FROM user_arcade_attrs WHERE user_id = 5653 AND activity_id = ? AND attr_id = 202").bind(activity_id).fetch_one(&pool).await.unwrap(), 190);
    }

    #[tokio::test]
    async fn reward_zero_claims_all_eligible_tiers_once() {
        let pool = test_pool(5654).await;
        let manager = ArcadeOutsideManager::new(5654);
        manager.info(&pool, config::configs::get()).await.unwrap();
        let activity_id = config::configs::get()
            .latest_open_activity_id(ACTIVITY_TYPE_ID)
            .unwrap();
        sqlx::query(
            "UPDATE user_arcade_outside SET score = 1000 WHERE user_id = 5654 AND activity_id = ?",
        )
        .bind(activity_id)
        .execute(&pool)
        .await
        .unwrap();

        let first = manager
            .gain_rewards(&pool, config::configs::get(), 0)
            .await
            .unwrap();
        assert!(!first.material_changes.is_empty());
        assert_eq!(first.red_dot_id, 3306);
        assert_eq!(
            arcade::get_claims(&pool, 5654, activity_id).await.unwrap(),
            vec![37001]
        );
        let second = manager
            .gain_rewards(&pool, config::configs::get(), 0)
            .await
            .unwrap();
        assert!(second.material_changes.is_empty());
        assert!(matches!(
            manager
                .gain_rewards(&pool, config::configs::get(), 37001)
                .await,
            Err(AppError::InvalidRequest)
        ));
    }
}
