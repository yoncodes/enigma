use super::{AppError, RougeManager, rouge2_info};
use database::db::game::rouge::{self, DifficultyUpdate};
use sonettobuf::{
    EnterRouge2SelectDifficultyReply, Rouge2Attr, Rouge2Bag, Rouge2BagInfo, Rouge2LeaderInfo,
};
use sqlx::SqlitePool;

pub(super) const EMPTY_STATE: i32 = 0;
pub(super) const DIFFICULTY_STATE: i32 = 1;

const RELICS_BAG: i32 = 1;
const BUFF_BAG: i32 = 3;
const ACTIVE_SKILL_BAG: i32 = 4;
const ACTIVE_SKILL_SLOTS: usize = 3;

pub struct DifficultyResult {
    pub reply: EnterRouge2SelectDifficultyReply,
    pub updates: Vec<Rouge2Attr>,
}

impl RougeManager {
    pub async fn select_difficulty(
        self,
        db: &SqlitePool,
        difficulty: i32,
    ) -> Result<DifficultyResult, AppError> {
        let tables = config::configs::get();
        let rows = rouge::get_or_create_rouge2_outside(db, self.player_id, tables).await?;
        let Some(difficulty_config) = tables
            .rouge2_difficulty
            .iter()
            .find(|row| row.difficulty == difficulty)
        else {
            return Err(AppError::InvalidRequest);
        };
        if rows.state.state != EMPTY_STATE || difficulty_config.pre_difficulty != 0 {
            return Err(AppError::InvalidRequest);
        }

        let attrs = common::config()
            .rouge2
            .initial_attributes
            .iter()
            .map(|attribute| {
                tables
                    .rouge2_attribute
                    .get(attribute.id)
                    .ok_or_else(|| {
                        AppError::Custom(format!(
                            "Rouge2 initial attribute {} is not configured",
                            attribute.id
                        ))
                    })
                    .map(|_| (attribute.id, attribute.value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if attrs.is_empty()
            || !rouge::apply_difficulty(
                db,
                self.player_id,
                DifficultyUpdate {
                    difficulty,
                    attrs: &attrs,
                },
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }

        let info = rouge2_info(db, self.player_id)
            .await?
            .rouge2_info
            .ok_or(AppError::InvalidRequest)?;
        let attributes = info.attr_info.as_ref().ok_or(AppError::InvalidRequest)?;
        let updates = common::config()
            .rouge2
            .initial_update_order
            .iter()
            .map(|attr_id| {
                attributes
                    .attr
                    .iter()
                    .find(|attribute| attribute.attr_id == Some(*attr_id))
                    .cloned()
                    .ok_or_else(|| {
                        AppError::Custom(format!(
                            "Rouge2 initial update attribute {attr_id} is missing"
                        ))
                    })
            })
            .collect::<Result<_, _>>()?;

        Ok(DifficultyResult {
            reply: EnterRouge2SelectDifficultyReply {
                rouge2_info: Some(info),
            },
            updates,
        })
    }
}

pub(super) fn initial_bag_info() -> Rouge2BagInfo {
    Rouge2BagInfo {
        bags: [RELICS_BAG, BUFF_BAG, ACTIVE_SKILL_BAG]
            .into_iter()
            .map(|bag_type| Rouge2Bag {
                bag_type: Some(bag_type),
                items: Vec::new(),
            })
            .collect(),
    }
}

pub(super) fn initial_leader_info() -> Rouge2LeaderInfo {
    Rouge2LeaderInfo {
        main_career_id: Some(0),
        career_id: Some(0),
        revival_coin: Some(0),
        equip_active_skill: vec![0; ACTIVE_SKILL_SLOTS],
        consume_attr_num: Some(0),
        summon_talent_id: Vec::new(),
        summon_talent_point: Some(0),
        system_team_id: Some(0),
        attr_drop_str: Some(String::new()),
    }
}
