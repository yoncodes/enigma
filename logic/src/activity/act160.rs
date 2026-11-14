use super::*;

const MISSION_CLAIMABLE: i32 = 1;
const MISSION_CLAIMED: i32 = 2;

pub struct Act160Claim {
    pub reply: Act160FinishMissionReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub updates: Vec<Act160MissionInfo>,
}

pub async fn act160_get_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Act160GetInfoReply, AppError> {
    let activity_id = activity_id.unwrap_or_else(latest_act160_activity_id);
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act160Mission).await?;
    let mut missions = config::configs::get()
        .activity160_mission
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .collect::<Vec<_>>();
    missions.sort_by_key(|row| (row.sort, row.id));

    Ok(Act160GetInfoReply {
        activity_id: Some(activity_id),
        act160_infos: missions
            .into_iter()
            .map(|row| Act160MissionInfo {
                id: Some(row.id),
                state: Some(states.get(&row.id).map(|(state, _, _)| *state).unwrap_or(0)),
            })
            .collect(),
    })
}

pub async fn finish_act160_mission(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    id: Option<i32>,
) -> Result<Act160Claim, AppError> {
    let activity_id = activity_id.unwrap_or_else(latest_act160_activity_id);
    let id = id.ok_or(AppError::InvalidRequest)?;
    let row = config::configs::get()
        .activity160_mission
        .get(id)
        .filter(|row| row.activity_id == activity_id)
        .ok_or(AppError::InvalidRequest)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act160Mission).await?;
    if states.get(&id).map(|(state, _, _)| *state).unwrap_or(0) != MISSION_CLAIMABLE {
        return Err(AppError::InvalidRequest);
    }

    activity_state::set(
        db,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act160Mission,
            entry_id: id,
            state: MISSION_CLAIMED,
            progress: 0,
            ext: "",
        },
    )
    .await?;
    let mut updates = vec![Act160MissionInfo {
        id: Some(id),
        state: Some(2),
    }];
    for next in config::configs::get()
        .activity160_mission
        .iter()
        .filter(|next| next.activity_id == activity_id && next.pre_id == id)
    {
        activity_state::set(
            db,
            player_id,
            activity_id,
            ActivityStateSet {
                kind: ActivityStateKind::Act160Mission,
                entry_id: next.id,
                state: MISSION_CLAIMABLE,
                progress: 0,
                ext: "",
            },
        )
        .await?;
        updates.push(Act160MissionInfo {
            id: Some(next.id),
            state: Some(1),
        });
    }

    let parsed = reward::parse(&row.bonus);
    let material_changes = parsed.material_changes();
    let rewards = reward::apply(db, player_id, parsed).await?;

    Ok(Act160Claim {
        reply: Act160FinishMissionReply {
            activity_id: Some(activity_id),
            act160_info: updates.first().copied(),
            is_read_mail: Some(false),
        },
        rewards,
        material_changes,
        updates,
    })
}
