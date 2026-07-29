use super::*;

pub struct Act125Claim {
    pub reply: FinishAct125EpisodeReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub async fn act125_infos(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct125InfosReply, AppError> {
    let activity_id = activity_id
        .or_else(default_act125_activity_id)
        .ok_or(AppError::InvalidRequest)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act125Episode).await?;
    let mut episodes = config::configs::get()
        .activity125
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| Act125Episode {
            id: Some(row.id),
            state: Some(states.get(&row.id).map(|(state, _, _)| *state).unwrap_or(0)),
        })
        .collect::<Vec<_>>();
    episodes.sort_by_key(|episode| episode.id.unwrap_or_default());

    Ok(GetAct125InfosReply {
        activity_id: Some(activity_id),
        act125_episodes: episodes,
    })
}

pub async fn finish_act125_episode(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    episode_id: Option<i32>,
    target_frequency: Option<i32>,
) -> Result<Act125Claim, AppError> {
    let activity_id = activity_id
        .or_else(default_act125_activity_id)
        .ok_or(AppError::InvalidRequest)?;
    let episode_id = episode_id.ok_or(AppError::InvalidRequest)?;
    let row = config::configs::get()
        .activity125
        .iter()
        .find(|row| row.activity_id == activity_id && row.id == episode_id)
        .ok_or(AppError::InvalidRequest)?;
    if target_frequency.is_some_and(|frequency| frequency != row.target_frequency) {
        return Err(AppError::InvalidRequest);
    }
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act125Episode).await?;
    if states
        .get(&episode_id)
        .map(|(state, _, _)| *state)
        .unwrap_or(0)
        == 1
    {
        return Err(AppError::InvalidRequest);
    }

    let expected_state = states
        .get(&episode_id)
        .map(|(state, _, _)| *state)
        .unwrap_or(0);
    let mut tx = db.begin().await?;
    if !activity_state::transition_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        expected_state,
        ActivityStateSet {
            kind: ActivityStateKind::Act125Episode,
            entry_id: episode_id,
            state: 1,
            progress: 0,
            ext: "",
        },
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }

    let parsed = reward::parse(&row.bonus);
    let material_changes = parsed.material_changes();
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, parsed).await?;
    tx.commit().await?;

    Ok(Act125Claim {
        reply: FinishAct125EpisodeReply {
            activity_id: Some(activity_id),
            episode_id: Some(episode_id),
            update_act125_episodes: vec![Act125Episode {
                id: Some(episode_id),
                state: Some(1),
            }],
        },
        rewards,
        material_changes,
    })
}
