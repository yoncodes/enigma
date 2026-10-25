use super::*;

mod episodes;
mod events;
mod overview;

#[derive(Clone, Debug)]
pub struct ActivityManager {
    pub(super) player_id: i64,
    states: HashMap<(i32, i32), activity_state::ActivityStates>,
}

impl ActivityManager {
    pub fn new(player_id: i64) -> Self {
        Self {
            player_id,
            states: HashMap::new(),
        }
    }

    async fn refresh_states(
        &mut self,
        db: &SqlitePool,
        activity_id: i32,
        kind: ActivityStateKind,
    ) -> Result<(), AppError> {
        let states = activity_state::get(db, self.player_id, activity_id, kind).await?;
        self.states.insert((activity_id, kind.id()), states);
        Ok(())
    }
}

async fn apply_activity_state(
    db: &SqlitePool,
    player_id: i64,
    infos: &mut [ActivityInfo],
) -> Result<(), AppError> {
    let new_stage =
        activity_state::get_activity_flags(db, player_id, ActivityStateKind::ActivityNewStage)
            .await?;
    let permanent_unlock = activity_state::get_activity_flags(
        db,
        player_id,
        ActivityStateKind::ActivityPermanentUnlock,
    )
    .await?;

    for info in infos {
        let Some(activity_id) = info.id.map(|id| id as i32) else {
            continue;
        };

        info.is_new_stage = Some(new_stage.contains(&activity_id));
        info.is_unlock =
            Some(is_unlocked_by_default(activity_id) || permanent_unlock.contains(&activity_id));
    }

    Ok(())
}
