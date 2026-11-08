use super::*;

const ACTIVITY_TYPE_ID: i32 = 107;

pub async fn act107_goods_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Get107GoodsInfoReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .ok_or(AppError::InvalidRequest)?;

    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act107Goods).await?;
    let goods_infos = tables
        .activity107
        .iter()
        .filter(|goods| goods.activity_id == activity_id && goods.is_online)
        .map(|goods| Act107GoodsInfo {
            id: Some(goods.id),
            buy_count: Some(
                states
                    .get(&goods.id)
                    .map(|(_, progress, _)| *progress)
                    .unwrap_or_default(),
            ),
        })
        .collect::<Vec<_>>();
    if goods_infos.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    Ok(Get107GoodsInfoReply {
        activity_id: Some(activity_id),
        goods_infos,
    })
}

#[cfg(test)]
mod tests;
