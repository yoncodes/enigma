use super::*;

const ACTIVITY_TYPE_ID: i32 = 227;

pub async fn party_outside_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetPartyOutSideInfoReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    config::configs::get()
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .ok_or(AppError::InvalidRequest)?;

    let states = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::PartyGameExtraBonus,
    )
    .await?;
    let have_extra_bonus_count = states
        .get(&0)
        .map(|(_, progress, _)| *progress)
        .unwrap_or_default();

    Ok(GetPartyOutSideInfoReply {
        activity_id: Some(activity_id),
        have_extra_bonus_count: Some(have_extra_bonus_count),
    })
}

#[cfg(test)]
mod tests;
