use super::ensure_act128_activity;
use crate::error::AppError;
use database::db::game::{activity128, currencies};
use sonettobuf::{Act128GalleryDetail, Act128GetExpReply, GetGalleryInfosReply};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub struct Act128ExpClaim {
    pub reply: Act128GetExpReply,
    pub currency_changes: Vec<(i32, i32)>,
}

async fn details(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
) -> Result<Vec<Act128GalleryDetail>, AppError> {
    let tables = config::configs::get();
    let claims = activity128::gallery_claims(tx, player_id).await?;
    let mut details = tables
        .activity128_gallery
        .iter()
        .filter(|row| row.offline == 0)
        .map(|row| {
            let saved = claims.get(&row.r#type);
            Act128GalleryDetail {
                r#type: Some(row.r#type),
                have_fight: Some(
                    saved.is_some_and(|(_, highest, played)| *highest > 0 || played == "1"),
                ),
                highest_point: Some(saved.map(|(_, highest, _)| *highest).unwrap_or(0)),
                accept_exp_point: Some(saved.map(|(accepted, _, _)| *accepted).unwrap_or(0)),
            }
        })
        .collect::<Vec<_>>();
    for (activity_id, stage, highest) in activity128::boss_scores(tx, player_id).await? {
        let Some(boss_type) = tables.activity128_boss_type(activity_id, stage) else {
            continue;
        };
        if let Some(detail) = details
            .iter_mut()
            .find(|detail| detail.r#type == Some(boss_type))
        {
            detail.highest_point = Some(detail.highest_point.unwrap_or(0).max(highest));
            detail.have_fight = Some(true);
        }
    }
    Ok(details)
}

pub async fn get_gallery_infos(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetGalleryInfosReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let mut tx = db.begin().await?;
    let gallery_detail = details(&mut tx, player_id).await?;
    tx.commit().await?;
    Ok(GetGalleryInfosReply {
        activity_id: Some(activity_id),
        gallery_detail,
    })
}

pub async fn get_act128_exp(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    boss_type: Option<i32>,
) -> Result<Act128ExpClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let boss_type = boss_type.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let tables = config::configs::get();
    let points = tables
        .activity128_gallery_points(boss_type)
        .ok_or(AppError::InvalidRequest)?;
    let currency_id = tables
        .activity128_rank_currency_id()
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    activity128::lock_gallery(&mut tx, player_id, boss_type).await?;
    let detail = details(&mut tx, player_id)
        .await?
        .into_iter()
        .find(|detail| detail.r#type == Some(boss_type) && detail.have_fight == Some(true))
        .ok_or(AppError::InvalidRequest)?;
    let previous = detail.accept_exp_point.unwrap_or(0);
    let highest = detail.highest_point.unwrap_or(0);
    let mut accepted = previous;
    let mut gained: i32 = 0;
    for (point, exp) in points {
        if point > previous && point <= highest {
            accepted = point;
            gained = gained.checked_add(exp).ok_or(AppError::InvalidRequest)?;
        }
    }
    let previous_exp = currencies::quantity_in_transaction(&mut tx, player_id, currency_id).await?;
    let player_exp = previous_exp
        .checked_add(gained)
        .filter(|exp| *exp >= 0)
        .ok_or(AppError::InvalidRequest)?;
    let player_level = tables
        .activity128_rank(player_exp)
        .ok_or(AppError::InvalidRequest)?;
    if !activity128::save_gallery(&mut tx, player_id, boss_type, previous, accepted, highest)
        .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let mut currency_changes = Vec::new();
    if gained > 0 {
        currencies::add_currency_in_transaction(
            &mut tx,
            player_id,
            currency_id,
            gained,
            common::time::ServerTime::now_ms(),
        )
        .await?;
        currency_changes.push((currency_id, gained));
    }
    tx.commit().await?;
    Ok(Act128ExpClaim {
        reply: Act128GetExpReply {
            activity_id: Some(activity_id),
            r#type: Some(boss_type),
            accept_exp_point: Some(accepted),
            player_level: Some(player_level),
            player_exp: Some(player_exp),
        },
        currency_changes,
    })
}

#[cfg(test)]
mod tests;
