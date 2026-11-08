use database::db::game::rouge::{self, GeniusActivation};
use sonettobuf::Rouge2ActiveGeniusReply;
use sqlx::SqlitePool;

use super::{AppError, RougeManager};

const COMMON_TALENT: i32 = 0;
const ORIGIN_ORDER: i32 = 1;
const EMPTY_STATE: i32 = 0;
const ENDED_STATE: i32 = 3;

pub struct GeniusResult {
    pub reply: Rouge2ActiveGeniusReply,
    pub genius_point: i32,
}

impl RougeManager {
    pub async fn activate_genius(
        self,
        db: &SqlitePool,
        genius_id: i32,
    ) -> Result<GeniusResult, AppError> {
        let tables = config::configs::get();
        let rows = rouge::get_or_create_rouge2_outside(db, self.player_id, tables).await?;
        if !matches!(rows.state.state, EMPTY_STATE | ENDED_STATE) {
            return Err(AppError::InvalidRequest);
        }
        let talent = tables
            .rouge2_genius_type
            .iter()
            .find(|row| row.genius_id == genius_id && row.talent == COMMON_TALENT)
            .filter(|row| row.order >= ORIGIN_ORDER && row.point_cost >= 0)
            .ok_or(AppError::InvalidRequest)?;
        if tables.rouge2_genius.get(genius_id).is_none() {
            return Err(AppError::InvalidRequest);
        }

        let mut genius_ids: Vec<i32> = serde_json::from_str(&rows.state.genius_ids)?;
        if genius_ids.contains(&genius_id) || rows.state.genius_point < talent.point_cost {
            return Err(AppError::InvalidRequest);
        }
        if talent.order > ORIGIN_ORDER {
            let predecessor = tables
                .rouge2_genius_type
                .iter()
                .find(|row| row.talent == COMMON_TALENT && row.order == talent.order - 1)
                .ok_or_else(|| {
                    AppError::Custom(format!(
                        "Rouge2 talent {genius_id} has no configured predecessor"
                    ))
                })?;
            if !genius_ids.contains(&predecessor.genius_id) {
                return Err(AppError::InvalidRequest);
            }
        }

        genius_ids.push(genius_id);
        let stored_ids = serde_json::to_string(&genius_ids)?;
        let genius_point = rouge::activate_genius(
            db,
            self.player_id,
            GeniusActivation {
                expected_state: rows.state.state,
                expected_ids: &rows.state.genius_ids,
                genius_ids: &stored_ids,
                point_cost: talent.point_cost,
            },
        )
        .await?
        .ok_or(AppError::InvalidRequest)?;

        Ok(GeniusResult {
            reply: Rouge2ActiveGeniusReply {
                genius_id: Some(genius_id),
                genius_ids,
            },
            genius_point,
        })
    }
}
