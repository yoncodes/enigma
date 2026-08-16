use crate::{error::AppError, reward};
use config::GameDB;
use database::{
    db::game::{arcade, tasks as task_db},
    models::game::tasks::UserTask,
};
use sonettobuf::prost::Message;
use sonettobuf::{
    ArcadeAttrContainer, ArcadeAttrValue, ArcadeBoardInfo, ArcadeBook, ArcadeBookInfo,
    ArcadeGainRewardReply, ArcadeGetInSideInfoReply, ArcadeGetOutSideInfoReply, ArcadeInSideInfo,
    ArcadeOutSideInfo, ArcadeOutSideProp, ArcadePlayer, ArcadePlayerMoveReply, ArcadePos,
    ArcadeSaveGameReply, ArcadeSettleGameReply, ArcadeSwitchCharacterReply, ArcadeTalent,
    ArcadeTalentInfo, ArcadeTalentUpgradeReply, ArcadeUnitInfo,
};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashSet};

const ACTIVITY_TYPE_ID: i32 = 222;
const DEFAULT_CHARACTER_CONST_ID: i32 = 1;
const DEFAULT_POSITION_CONST_ID: i32 = 3;
const HALL_SIZE_CONST_ID: i32 = 4;
const DIAMOND_ATTR_ID: i32 = 202;
const CASSETTE_ATTR_ID: i32 = 207;
const SETTLEMENT_FACTOR_CONST_ID: i32 = 20;
const CHARACTER_BOOK_SCORE_CONST_ID: i32 = 21;
const COLLECTION_BOOK_SCORE_CONST_ID: i32 = 22;
const MONSTER_BOOK_SCORE_CONST_ID: i32 = 23;
const FLOOR_BOOK_SCORE_CONST_ID: i32 = 24;

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

pub struct ArcadeSettlement {
    pub reply: ArcadeSettleGameReply,
    pub changed_attr: ArcadeAttrValue,
    pub tasks: Vec<UserTask>,
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

    pub async fn inside_info(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
    ) -> Result<ArcadeGetInSideInfoReply, AppError> {
        let activity_id = self.ensure_state(db, tables).await?;
        let Some(saved) = arcade::get_inside_save(db, self.player_id, activity_id).await? else {
            return Ok(ArcadeGetInSideInfoReply {
                info: None,
                has_save_game: Some(false),
            });
        };
        let info = ArcadeInSideInfo::decode(saved.snapshot.as_slice())
            .map_err(|_| AppError::InvalidRequest)?;
        if info.prop.as_ref().and_then(|prop| prop.difficulty) != Some(saved.difficulty) {
            return Err(AppError::InvalidRequest);
        }
        Ok(ArcadeGetInSideInfoReply {
            info: Some(info),
            has_save_game: Some(true),
        })
    }

    pub async fn save_inside(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        info: ArcadeInSideInfo,
    ) -> Result<ArcadeSaveGameReply, AppError> {
        let activity_id = self.ensure_state(db, tables).await?;
        let difficulty = validate_inside_info(tables, &info, true)?;
        if !self
            .difficulty_is_available(db, activity_id, tables, difficulty)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        let snapshot = info.encode_to_vec();
        if snapshot.len() > 1024 * 1024 {
            return Err(AppError::InvalidRequest);
        }
        run_cursor(&info)?;
        let saved = arcade::get_inside_save(db, self.player_id, activity_id).await?;
        if let Some(saved) = &saved {
            let saved_info = ArcadeInSideInfo::decode(saved.snapshot.as_slice())
                .map_err(|_| AppError::InvalidRequest)?;
            validate_save_successor(&saved_info, &info)?;
        }
        if !arcade::replace_inside_save(
            db,
            self.player_id,
            activity_id,
            difficulty,
            &snapshot,
            common::time::ServerTime::now_ms(),
            saved.as_ref().map(|saved| saved.snapshot.as_slice()),
        )
        .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(ArcadeSaveGameReply {})
    }

    pub async fn settle_inside(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        settle_type: i32,
        info: ArcadeInSideInfo,
    ) -> Result<ArcadeSettlement, AppError> {
        if !(1..=3).contains(&settle_type) {
            return Err(AppError::InvalidRequest);
        }
        let is_win = settle_type == 2;
        let activity_id = self.ensure_state(db, tables).await?;
        let difficulty = validate_inside_info(tables, &info, false)?;
        if !self
            .difficulty_is_available(db, activity_id, tables, difficulty)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        let saved = arcade::get_inside_save(db, self.player_id, activity_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let saved_info = ArcadeInSideInfo::decode(saved.snapshot.as_slice())
            .map_err(|_| AppError::InvalidRequest)?;
        if saved.difficulty != difficulty {
            return Err(AppError::InvalidRequest);
        }
        validate_settlement_successor(&saved_info, &info)?;

        let attrs = info
            .attr_container
            .as_ref()
            .ok_or(AppError::InvalidRequest)?;
        let diamond_gain = attr_base(attrs, DIAMOND_ATTR_ID)?;
        let cassette = attr_base(attrs, CASSETTE_ATTR_ID)?;
        if diamond_gain < 0 || cassette < 0 {
            return Err(AppError::InvalidRequest);
        }
        let factor = settlement_factor(
            self.const_value(tables, SETTLEMENT_FACTOR_CONST_ID)?,
            difficulty,
        )?;
        let cassette_score = ((cassette as f64) * factor).floor();
        if !(0.0..=i32::MAX as f64).contains(&cassette_score) {
            return Err(AppError::InvalidRequest);
        }

        let extend = info.extend_info.as_ref().ok_or(AppError::InvalidRequest)?;
        let books = validate_books(tables, extend.added_book.as_ref())?;
        let mut seen_roles = HashSet::new();
        let roles = extend
            .unlock_role_ids
            .iter()
            .copied()
            .filter(|id| seen_roles.insert(*id))
            .map(|id| {
                tables
                    .arcade_character
                    .get(id)
                    .filter(|row| row.category == "character")
                    .map(|_| id)
                    .ok_or(AppError::InvalidRequest)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unlock_difficulty = if is_win {
            let next = difficulty.checked_add(1).ok_or(AppError::InvalidRequest)?;
            tables
                .arcade_difficulty
                .iter()
                .any(|row| row.level == next)
                .then_some(next)
        } else {
            None
        };
        if !is_win && !extend.unlock_difficulty_ids.is_empty()
            || extend
                .unlock_difficulty_ids
                .iter()
                .any(|id| Some(*id) != unlock_difficulty)
        {
            return Err(AppError::InvalidRequest);
        }

        let diamond = tables
            .arcade_attribute
            .get(DIAMOND_ATTR_ID)
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
        let mut tx = db.begin().await?;
        let result = arcade::settle_inside_in_transaction(
            &mut tx,
            self.player_id,
            activity_id,
            &saved.snapshot,
            arcade::SettleProjection {
                difficulty,
                diamond_gain,
                diamond_min: diamond.min,
                diamond_max: diamond.max,
                cassette_score: cassette_score as i32,
                books,
                roles,
                unlock_difficulty,
                is_win,
                updated_at: common::time::ServerTime::now_ms(),
            },
        )
        .await?
        .ok_or(AppError::InvalidRequest)?;
        let tasks = if is_win {
            task_db::sync_event_tasks_in_transaction(
                &mut tx,
                self.player_id,
                task_db::TaskEvent::Act222ArcadeSettle { activity_id },
            )
            .await?
        } else {
            Vec::new()
        };
        tx.commit().await?;

        Ok(ArcadeSettlement {
            reply: ArcadeSettleGameReply {
                book_add_score: Some(result.book_score),
                unlock_role_ids: result.new_roles,
                hotfix: None,
            },
            changed_attr: attr_message(result.attr),
            tasks,
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

    async fn difficulty_is_available(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        tables: &GameDB,
        difficulty: i32,
    ) -> Result<bool, AppError> {
        if difficulty == 0 {
            return Ok(true);
        }
        if !tables
            .arcade_difficulty
            .iter()
            .any(|row| row.level == difficulty)
        {
            return Ok(false);
        }
        Ok(
            arcade::get_unlock_difficulties(db, self.player_id, activity_id)
                .await?
                .contains(&difficulty),
        )
    }

    fn const_value<'a>(&self, tables: &'a GameDB, id: i32) -> Result<&'a str, AppError> {
        tables
            .arcade_const
            .get(id)
            .map(|row| row.value.as_str())
            .ok_or(AppError::InvalidRequest)
    }
}

fn validate_inside_info(
    tables: &GameDB,
    info: &ArcadeInSideInfo,
    full_save: bool,
) -> Result<i32, AppError> {
    let player = info.player.as_ref();
    if full_save {
        let player = player.ok_or(AppError::InvalidRequest)?;
        let character_id = player.id.ok_or(AppError::InvalidRequest)?;
        if tables.arcade_character.get(character_id).is_none()
            || info.collectible_slots.len() > 256
            || info.passive_skill_ids.len() > 256
        {
            return Err(AppError::InvalidRequest);
        }
    }
    let prop = info.prop.as_ref().ok_or(AppError::InvalidRequest)?;
    let difficulty = prop.difficulty.ok_or(AppError::InvalidRequest)?;
    if difficulty < 0 || info.extend_info.is_none() {
        return Err(AppError::InvalidRequest);
    }
    let attrs = info
        .attr_container
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    if attrs.attr_values.is_empty() || attrs.attr_values.len() > 128 {
        return Err(AppError::InvalidRequest);
    }
    let mut seen = HashSet::new();
    for attr in &attrs.attr_values {
        let id = attr.id.ok_or(AppError::InvalidRequest)?;
        let config = tables
            .arcade_attribute
            .get(id)
            .ok_or(AppError::InvalidRequest)?;
        let base = attr.base.ok_or(AppError::InvalidRequest)?;
        if !seen.insert(id) || base < config.min || base > config.max {
            return Err(AppError::InvalidRequest);
        }
    }
    Ok(difficulty)
}

fn attr_base(attrs: &ArcadeAttrContainer, id: i32) -> Result<i32, AppError> {
    attrs
        .attr_values
        .iter()
        .find(|attr| attr.id == Some(id))
        .and_then(|attr| attr.base)
        .ok_or(AppError::InvalidRequest)
}

fn validate_settlement_successor(
    saved: &ArcadeInSideInfo,
    settlement: &ArcadeInSideInfo,
) -> Result<(), AppError> {
    if run_cursor(settlement)? < run_cursor(saved)? {
        return Err(AppError::InvalidRequest);
    }
    let saved_attrs = saved
        .attr_container
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    let settlement_attrs = settlement
        .attr_container
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    for attr_id in [DIAMOND_ATTR_ID, CASSETTE_ATTR_ID] {
        if attr_base(settlement_attrs, attr_id)? < attr_base(saved_attrs, attr_id)? {
            return Err(AppError::InvalidRequest);
        }
    }

    let saved_prop = saved.prop.as_ref().ok_or(AppError::InvalidRequest)?;
    let settlement_prop = settlement.prop.as_ref().ok_or(AppError::InvalidRequest)?;
    if saved_prop.difficulty != settlement_prop.difficulty {
        return Err(AppError::InvalidRequest);
    }
    for (saved_value, settlement_value) in [
        (
            saved_prop.max_kill_monster_num,
            settlement_prop.max_kill_monster_num,
        ),
        (
            saved_prop.total_gain_gold_num,
            settlement_prop.total_gain_gold_num,
        ),
        (saved_prop.highest_score, settlement_prop.highest_score),
        (
            saved_prop.cleared_room_num,
            settlement_prop.cleared_room_num,
        ),
    ] {
        if saved_value.is_some_and(|value| settlement_value.is_none_or(|next| next < value)) {
            return Err(AppError::InvalidRequest);
        }
    }

    let saved_extend = saved.extend_info.as_ref().ok_or(AppError::InvalidRequest)?;
    let settlement_extend = settlement
        .extend_info
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    let settlement_books = book_entries(settlement_extend.added_book.as_ref())?;
    if !book_entries(saved_extend.added_book.as_ref())?.is_subset(&settlement_books)
        || !saved_extend
            .unlock_role_ids
            .iter()
            .all(|id| settlement_extend.unlock_role_ids.contains(id))
    {
        return Err(AppError::InvalidRequest);
    }

    Ok(())
}

fn validate_save_successor(
    saved: &ArcadeInSideInfo,
    next: &ArcadeInSideInfo,
) -> Result<(), AppError> {
    if saved == next {
        return Ok(());
    }
    validate_settlement_successor(saved, next)?;
    if run_cursor(next)? <= run_cursor(saved)? {
        return Err(AppError::InvalidRequest);
    }
    Ok(())
}

fn run_cursor(info: &ArcadeInSideInfo) -> Result<(i32, i32), AppError> {
    let prop = info.prop.as_ref().ok_or(AppError::InvalidRequest)?;
    let area = prop.area_id.ok_or(AppError::InvalidRequest)?;
    let progress = prop.progress.ok_or(AppError::InvalidRequest)?;
    if area < 0 || progress < 0 {
        return Err(AppError::InvalidRequest);
    }
    Ok((area, progress))
}

fn book_entries(info: Option<&ArcadeBookInfo>) -> Result<HashSet<(i32, i32)>, AppError> {
    info.into_iter()
        .flat_map(|info| &info.books)
        .flat_map(|book| {
            book.ele_id
                .iter()
                .map(move |id| book.r#type.map(|book_type| (book_type, *id)))
        })
        .collect::<Option<HashSet<_>>>()
        .ok_or(AppError::InvalidRequest)
}

fn settlement_factor(value: &str, difficulty: i32) -> Result<f64, AppError> {
    if difficulty == 0 {
        return Ok(1.0);
    }
    value
        .split('#')
        .nth((difficulty - 1) as usize)
        .and_then(|part| part.parse::<f64>().ok())
        .filter(|factor| factor.is_finite() && *factor >= 0.0)
        .ok_or(AppError::InvalidRequest)
}

fn validate_books(
    tables: &GameDB,
    info: Option<&ArcadeBookInfo>,
) -> Result<Vec<arcade::SettleBook>, AppError> {
    let scores = [
        (1, CHARACTER_BOOK_SCORE_CONST_ID),
        (2, COLLECTION_BOOK_SCORE_CONST_ID),
        (3, FLOOR_BOOK_SCORE_CONST_ID),
        (4, MONSTER_BOOK_SCORE_CONST_ID),
    ]
    .into_iter()
    .map(|(book_type, const_id)| {
        let score = tables
            .arcade_const
            .get(const_id)
            .and_then(|row| row.value.parse::<i32>().ok())
            .filter(|score| *score >= 0)
            .ok_or(AppError::InvalidRequest)?;
        Ok((book_type, score))
    })
    .collect::<Result<BTreeMap<_, _>, AppError>>()?;

    let mut seen = HashSet::new();
    let mut books = Vec::new();
    for book in info.into_iter().flat_map(|info| &info.books) {
        let book_type = book.r#type.ok_or(AppError::InvalidRequest)?;
        let score = *scores.get(&book_type).ok_or(AppError::InvalidRequest)?;
        for id in &book.ele_id {
            let category_matches = match book_type {
                1 => tables
                    .arcade_character
                    .get(*id)
                    .map(|row| row.category == "character"),
                2 => tables
                    .arcade_collection
                    .get(*id)
                    .map(|row| row.category == "item"),
                3 => tables
                    .arcade_floor
                    .get(*id)
                    .map(|row| row.category == "floor"),
                4 => tables
                    .arcade_monster
                    .get(*id)
                    .map(|row| row.category == "monster"),
                _ => None,
            };
            let Some(category_matches) = category_matches else {
                return Err(AppError::InvalidRequest);
            };
            if !category_matches {
                continue;
            }
            if !seen.insert((book_type, *id)) {
                return Err(AppError::InvalidRequest);
            }
            books.push(arcade::SettleBook {
                book_type,
                element_id: *id,
                score,
            });
        }
    }
    Ok(books)
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
    use sonettobuf::{ArcadeExtendInfo, ArcadeInSideProp};

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

    fn inside_run(difficulty: i32, diamond: i32, cassette: i32) -> ArcadeInSideInfo {
        ArcadeInSideInfo {
            player: Some(ArcadePlayer {
                id: Some(101),
                ..Default::default()
            }),
            attr_container: Some(ArcadeAttrContainer {
                attr_values: vec![
                    ArcadeAttrValue {
                        id: Some(DIAMOND_ATTR_ID),
                        base: Some(diamond),
                        ..Default::default()
                    },
                    ArcadeAttrValue {
                        id: Some(CASSETTE_ATTR_ID),
                        base: Some(cassette),
                        ..Default::default()
                    },
                ],
            }),
            prop: Some(ArcadeInSideProp {
                area_id: Some(0),
                room_id: Some(10001),
                progress: Some(0),
                difficulty: Some(difficulty),
                ..Default::default()
            }),
            extend_info: Some(ArcadeExtendInfo {
                added_book: Some(ArcadeBookInfo {
                    books: vec![
                        ArcadeBook {
                            r#type: Some(2),
                            ele_id: vec![10009, 10004],
                            ..Default::default()
                        },
                        ArcadeBook {
                            r#type: Some(3),
                            ele_id: vec![106],
                            ..Default::default()
                        },
                        ArcadeBook {
                            r#type: Some(4),
                            ele_id: vec![
                                200015, 200007, 210002, 200001, 210004, 200004, 200002, 200003,
                                200005, 200008, 210001, 210003, 200006,
                            ],
                            ..Default::default()
                        },
                    ],
                }),
                unlock_difficulty_ids: vec![difficulty + 1],
                ..Default::default()
            }),
            ..Default::default()
        }
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

    #[tokio::test]
    async fn inside_save_round_trips_and_settlement_is_atomic() {
        let pool = test_pool(5671).await;
        let manager = ArcadeOutsideManager::new(5671);
        let tables = config::configs::get();
        assert_eq!(
            manager
                .inside_info(&pool, tables)
                .await
                .unwrap()
                .has_save_game,
            Some(false)
        );

        let checkpoint = inside_run(0, 90, 1500);
        let run = inside_run(0, 240, 2500);
        manager
            .save_inside(&pool, tables, checkpoint.clone())
            .await
            .unwrap();
        let saved = manager.inside_info(&pool, tables).await.unwrap();
        assert_eq!(saved.has_save_game, Some(true));
        assert_eq!(saved.info, Some(checkpoint));
        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        sqlx::query(
            "INSERT INTO user_arcade_attrs
             (user_id, activity_id, attr_id, base, rate, extra)
             VALUES (5671, ?, 202, 100, 0, 0)",
        )
        .bind(activity_id)
        .execute(&pool)
        .await
        .unwrap();

        let settlement = manager
            .settle_inside(&pool, tables, 2, run.clone())
            .await
            .unwrap();
        assert_eq!(settlement.reply.book_add_score, Some(400));
        assert_eq!(settlement.reply.unlock_role_ids, Vec::<i32>::new());
        assert_eq!(settlement.reply.hotfix, None);
        assert_eq!(settlement.changed_attr.base, Some(340));
        assert!(settlement.tasks.iter().any(|task| {
            task.type_id == task_db::TaskType::VersionActivity.id() && task.task_id == 912
        }));
        assert!(settlement.tasks.iter().any(|task| {
            task.type_id == task_db::TaskType::ActBp.id() && task.task_id == 790012
        }));
        assert_eq!(settlement.red_dot_id, 3306);

        let state = arcade::get_state(&pool, 5671, activity_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.score, 2900);
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&state.hotfix).unwrap(),
            vec!["0#1"]
        );
        assert!(
            arcade::get_unlock_difficulties(&pool, 5671, activity_id)
                .await
                .unwrap()
                .contains(&1)
        );
        assert!(
            arcade::get_inside_save(&pool, 5671, activity_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            manager.settle_inside(&pool, tables, 2, run).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            arcade::get_state(&pool, 5671, activity_id)
                .await
                .unwrap()
                .unwrap()
                .score,
            2900
        );
    }

    #[tokio::test]
    async fn settlement_rejects_a_stale_same_difficulty_checkpoint() {
        let pool = test_pool(5672).await;
        let manager = ArcadeOutsideManager::new(5672);
        let tables = config::configs::get();
        manager.info(&pool, tables).await.unwrap();
        let stale = inside_run(0, 90, 1500);
        let mut current = inside_run(0, 240, 2500);
        current.prop.as_mut().unwrap().progress = Some(1);
        manager
            .save_inside(&pool, tables, stale.clone())
            .await
            .unwrap();
        manager
            .save_inside(&pool, tables, current.clone())
            .await
            .unwrap();

        assert!(matches!(
            manager.save_inside(&pool, tables, stale.clone()).await,
            Err(AppError::InvalidRequest)
        ));

        assert!(matches!(
            manager.settle_inside(&pool, tables, 2, stale).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            manager.inside_info(&pool, tables).await.unwrap().info,
            Some(current)
        );
        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        assert_eq!(
            arcade::get_state(&pool, 5672, activity_id)
                .await
                .unwrap()
                .unwrap()
                .score,
            0
        );
    }

    #[tokio::test]
    async fn delayed_save_with_equal_totals_cannot_replace_a_later_room() {
        let pool = test_pool(5676).await;
        let manager = ArcadeOutsideManager::new(5676);
        let tables = config::configs::get();
        let previous = inside_run(0, 90, 1500);
        let mut current = previous.clone();
        let current_prop = current.prop.as_mut().unwrap();
        current_prop.room_id = Some(10002);
        current_prop.progress = Some(1);
        current.player.as_mut().unwrap().pos = Some(ArcadePos {
            x: Some(3),
            y: Some(4),
        });

        manager
            .save_inside(&pool, tables, previous.clone())
            .await
            .unwrap();
        manager
            .save_inside(&pool, tables, current.clone())
            .await
            .unwrap();
        manager
            .save_inside(&pool, tables, current.clone())
            .await
            .unwrap();
        assert!(matches!(
            manager.save_inside(&pool, tables, previous).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            manager.inside_info(&pool, tables).await.unwrap().info,
            Some(current.clone())
        );

        let mut next_area = current;
        let next_prop = next_area.prop.as_mut().unwrap();
        next_prop.area_id = Some(1);
        next_prop.room_id = Some(20001);
        next_prop.progress = Some(0);
        manager
            .save_inside(&pool, tables, next_area.clone())
            .await
            .unwrap();
        assert_eq!(
            manager.inside_info(&pool, tables).await.unwrap().info,
            Some(next_area)
        );
    }

    #[tokio::test]
    async fn task_failure_rolls_back_settlement_and_keeps_the_save() {
        let pool = test_pool(5673).await;
        let manager = ArcadeOutsideManager::new(5673);
        let tables = config::configs::get();
        let checkpoint = inside_run(0, 90, 1500);
        let run = inside_run(0, 240, 2500);
        manager
            .save_inside(&pool, tables, checkpoint)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_arcade_task_progress
             BEFORE UPDATE OF progress ON user_tasks
             WHEN NEW.user_id = 5673 AND NEW.task_id = 912
             BEGIN SELECT RAISE(FAIL, 'forced task failure'); END",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert!(manager.settle_inside(&pool, tables, 2, run).await.is_err());
        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        assert!(
            arcade::get_inside_save(&pool, 5673, activity_id)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            arcade::get_state(&pool, 5673, activity_id)
                .await
                .unwrap()
                .unwrap()
                .score,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_arcade_attrs
                 WHERE user_id = 5673 AND activity_id = ? AND attr_id = 202",
            )
            .bind(activity_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn difficulty_factors_keep_float_precision_until_score_settlement() {
        let pool = test_pool(5674).await;
        let manager = ArcadeOutsideManager::new(5674);
        let tables = config::configs::get();

        manager
            .save_inside(&pool, tables, inside_run(1, 300, 3500))
            .await
            .unwrap();
        manager
            .settle_inside(&pool, tables, 2, inside_run(1, 450, 4500))
            .await
            .unwrap();
        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        assert_eq!(
            arcade::get_state(&pool, 5674, activity_id)
                .await
                .unwrap()
                .unwrap()
                .score,
            4900
        );

        manager
            .save_inside(&pool, tables, inside_run(2, 450, 4500))
            .await
            .unwrap();
        manager
            .settle_inside(&pool, tables, 2, inside_run(2, 600, 4750))
            .await
            .unwrap();
        assert_eq!(
            arcade::get_state(&pool, 5674, activity_id)
                .await
                .unwrap()
                .unwrap()
                .score,
            11_075
        );
    }

    #[tokio::test]
    async fn fail_and_abandon_do_not_increment_completion_or_tasks() {
        let pool = test_pool(5675).await;
        let manager = ArcadeOutsideManager::new(5675);
        let tables = config::configs::get();
        let mut fail = inside_run(0, 90, 1500);
        fail.extend_info
            .as_mut()
            .unwrap()
            .unlock_difficulty_ids
            .clear();
        manager
            .save_inside(&pool, tables, fail.clone())
            .await
            .unwrap();
        assert!(
            manager
                .settle_inside(&pool, tables, 3, fail)
                .await
                .unwrap()
                .tasks
                .is_empty()
        );

        let mut abandon = inside_run(0, 120, 2000);
        abandon
            .extend_info
            .as_mut()
            .unwrap()
            .unlock_difficulty_ids
            .clear();
        manager
            .save_inside(&pool, tables, abandon.clone())
            .await
            .unwrap();
        assert!(
            manager
                .settle_inside(&pool, tables, 1, abandon)
                .await
                .unwrap()
                .tasks
                .is_empty()
        );

        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_arcade_completions
                 WHERE user_id = 5675 AND activity_id = ?",
            )
            .bind(activity_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            serde_json::from_str::<Vec<String>>(
                &arcade::get_state(&pool, 5675, activity_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .hotfix,
            )
            .unwrap(),
            vec![""]
        );
    }
}
