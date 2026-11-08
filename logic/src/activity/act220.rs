use super::*;

const ACTIVITY_TYPE_ID: i32 = 220;

pub struct Act220Finish {
    pub reply: Act220FinishEpisodeReply,
    pub episode_push: Act220EpisodePush,
    pub tasks: Vec<crate::task::UserTask>,
}

pub async fn act220_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct220InfoReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id
        .or_else(|| tables.latest_open_activity_id(ACTIVITY_TYPE_ID))
        .ok_or(AppError::InvalidRequest)?;
    validate_activity(activity_id)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act220Episode).await?;
    let mut episodes = tables
        .activity220_episode
        .iter()
        .filter(|episode| {
            episode.activity_id == activity_id
                && (episode.pre_episode_id == 0
                    || (episode.pre_episode_branch_id == 0
                        && states
                            .get(&episode.pre_episode_id)
                            .is_some_and(|(state, _, _)| *state == 1)))
        })
        .map(|episode| episode_record(episode.episode_id, states.get(&episode.episode_id)))
        .collect::<Vec<_>>();
    episodes.sort_by_key(|episode| episode.episode_id);
    if episodes.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    Ok(GetAct220InfoReply {
        activity_id: Some(activity_id),
        episodes,
    })
}

pub async fn finish_act220_episode(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    episode_id: Option<i32>,
    progress: Option<String>,
) -> Result<Act220Finish, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let episode_id = episode_id.ok_or(AppError::InvalidRequest)?;
    let progress = progress.unwrap_or_default();
    validate_activity(activity_id)?;
    let tables = config::configs::get();
    let episode = tables
        .activity220_episode
        .iter()
        .find(|episode| episode.activity_id == activity_id && episode.episode_id == episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act220Episode).await?;
    if states
        .get(&episode_id)
        .is_some_and(|(state, _, _)| *state == 1)
        || (episode.pre_episode_id != 0
            && (episode.pre_episode_branch_id != 0
                || !states
                    .get(&episode.pre_episode_id)
                    .is_some_and(|(state, _, _)| *state == 1)))
    {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    if !activity_state::insert_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act220Episode,
            entry_id: episode_id,
            state: 1,
            progress: 0,
            ext: &progress,
        },
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let tasks = database::db::game::tasks::sync_event_tasks_in_transaction(
        &mut tx,
        player_id,
        crate::task::TaskEvent::Act220 {
            activity_id,
            episode_id,
        },
    )
    .await?;
    tx.commit().await?;

    let mut episodes = tables
        .activity220_episode
        .iter()
        .filter(|next| {
            next.activity_id == activity_id
                && next.pre_episode_id == episode_id
                && next.pre_episode_branch_id == 0
        })
        .map(|next| episode_record(next.episode_id, None))
        .collect::<Vec<_>>();
    episodes.sort_by_key(|episode| episode.episode_id);

    Ok(Act220Finish {
        reply: Act220FinishEpisodeReply {
            activity_id: Some(activity_id),
            episode_id: Some(episode_id),
            progress: Some(progress),
        },
        episode_push: Act220EpisodePush {
            activity_id: Some(activity_id),
            episodes,
        },
        tasks,
    })
}

fn validate_activity(activity_id: i32) -> Result<(), AppError> {
    config::configs::get()
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .map(|_| ())
        .ok_or(AppError::InvalidRequest)
}

fn episode_record(episode_id: i32, state: Option<&(i32, i32, String)>) -> Act220EpisodeRecord {
    Act220EpisodeRecord {
        episode_id: Some(episode_id),
        is_finished: Some(state.is_some_and(|(state, _, _)| *state == 1)),
        unlock_branch_ids: Vec::new(),
        progress: Some(
            state
                .map(|(_, _, progress)| progress.clone())
                .unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests;
