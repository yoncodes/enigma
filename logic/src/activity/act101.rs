use super::*;

pub struct Activity101Claim {
    pub reply: Get101BonusReply,
    pub rewards: Option<reward::AppliedRewards>,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub has_claimable: bool,
}

pub struct Activity101SpClaim {
    pub reply: Get101SpBonusReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub has_claimable: bool,
}

pub async fn get101_infos(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Get101InfosReply, AppError> {
    let activity_id = activity_id.unwrap_or_else(latest_act101_activity_id);
    let (infos, login_count, got_once_bonus) =
        activity101::get_activity101_info(db, player_id, activity_id).await?;

    Ok(Get101InfosReply {
        infos: infos
            .into_iter()
            .map(|(id, state)| Act101Info {
                id: Some(id as u32),
                state: Some(state as u32),
            })
            .collect(),
        login_count: Some(login_count as u32),
        activity_id: Some(activity_id),
        sp_infos: activity101_sp_infos(db, player_id, activity_id, login_count).await?,
        got_once_bonus: Some(got_once_bonus),
    })
}

pub async fn get101_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    day_id: Option<u32>,
) -> Result<Activity101Claim, AppError> {
    let activity_id = activity_id.unwrap_or_else(latest_act101_activity_id);
    let day_id = day_id.unwrap_or_default() as i32;
    let row = config::configs::get()
        .activity101
        .iter()
        .find(|row| row.activity_id == activity_id && row.id == day_id)
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let claimed =
        activity101::claim_activity101_day_in_transaction(&mut tx, player_id, activity_id, day_id)
            .await?;

    let mut rewards = None;
    let mut material_changes = Vec::new();
    if claimed {
        let parsed = reward::parse(&row.bonus);
        material_changes = parsed.material_changes();
        rewards = Some(reward::apply_in_transaction(&mut tx, db, player_id, parsed).await?);
    }
    tx.commit().await?;

    let has_claimable = activity101::get_activity101_info(db, player_id, activity_id)
        .await?
        .0
        .iter()
        .any(|(_, state)| *state == 1);

    Ok(Activity101Claim {
        reply: Get101BonusReply {
            id: Some(day_id as u32),
            activity_id: Some(activity_id),
        },
        rewards,
        material_changes,
        has_claimable,
    })
}

pub async fn get101_sp_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    id: Option<i32>,
) -> Result<Activity101SpClaim, AppError> {
    let activity_id = activity_id.unwrap_or_else(latest_act101_activity_id);
    let id = id.ok_or(AppError::InvalidRequest)?;
    let row = config::configs::get()
        .activity101_sp_bonus
        .iter()
        .find(|row| row.activity_id == activity_id && row.id == id)
        .ok_or(AppError::InvalidRequest)?;
    let (_, login_count, _) = activity101::get_activity101_info(db, player_id, activity_id).await?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act101SpBonus).await?;
    let state = sp_bonus_state(
        states.get(&id).map(|(state, _, _)| *state),
        login_count,
        row,
    );
    if state != 1 {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    if !activity_state::transition_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        states.get(&id).map(|(state, _, _)| *state).unwrap_or(0),
        ActivityStateSet {
            kind: ActivityStateKind::Act101SpBonus,
            entry_id: id,
            state: 2,
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
    let has_claimable = activity101_sp_infos(db, player_id, activity_id, login_count)
        .await?
        .iter()
        .any(|info| info.state == Some(1));

    Ok(Activity101SpClaim {
        reply: Get101SpBonusReply {
            id: Some(id),
            activity_id: Some(activity_id),
        },
        rewards,
        material_changes,
        has_claimable,
    })
}

async fn activity101_sp_infos(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    login_count: i32,
) -> Result<Vec<Act101SpInfo>, AppError> {
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act101SpBonus).await?;
    Ok(config::configs::get()
        .activity101_sp_bonus
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| Act101SpInfo {
            id: Some(row.id),
            state: Some(sp_bonus_state(
                states.get(&row.id).map(|(state, _, _)| *state),
                login_count,
                row,
            )),
        })
        .collect())
}

fn sp_bonus_state(
    stored: Option<i32>,
    login_count: i32,
    row: &config::activity101_sp_bonus::Activity101SpBonus,
) -> i32 {
    if stored == Some(2) {
        2
    } else if login_count >= row.can_get_sign_in_days {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stored_claimable_day_transitions_once() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (7, 'act101', 0, 0);
             INSERT INTO user_sign_in_info
                (user_id, addup_sign_in_day, open_function_time, reward_mark)
             VALUES (7, 1, 0, 0);
             INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (7, 101, ?, 1, 1, 0, '', 0);",
        )
        .bind(ActivityStateKind::Act101Day.id())
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let claimed = activity101::claim_activity101_day_in_transaction(&mut tx, 7, 101, 1)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let mut retry = pool.begin().await.unwrap();
        let claimed_again =
            activity101::claim_activity101_day_in_transaction(&mut retry, 7, 101, 1)
                .await
                .unwrap();
        retry.commit().await.unwrap();

        assert!(claimed);
        assert!(!claimed_again);
    }
}
