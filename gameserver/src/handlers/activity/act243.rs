use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{
    Act243ReportEpisodeRequest, Act243UnlockRacerRequest, Act243UpgradeGiftRequest, CmdId,
    GetAct243InfoRequest,
};

pub async fn on_get_act243_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct243InfoRequest::decode(&req.data[..])?;
    let activity_id = msg.activity_id.ok_or(AppError::InvalidRequest)?;
    let db = ctx.state.db;
    let reply = ctx.player()?.activity.act243_info(db, activity_id).await?;

    ctx.send_reply(CmdId::GetAct243InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_act243_report_episode(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act243ReportEpisodeRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let report = ctx
        .player()?
        .activity
        .report_act243_episode(
            db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.episode_id.ok_or(AppError::InvalidRequest)?,
            msg.score.ok_or(AppError::InvalidRequest)?,
            msg.star.ok_or(AppError::InvalidRequest)?,
            msg.time_ms.ok_or(AppError::InvalidRequest)?,
            msg.record.unwrap_or_default(),
        )
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        report.rewards,
        report.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(CmdId::Act243ReportEpisodeCmd, report.reply, 0, req.up_tag)
        .await
}

pub async fn on_act243_upgrade_gift(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act243UpgradeGiftRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let result = ctx
        .player()?
        .activity
        .upgrade_act243_gift(
            db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.gift_point.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        result.consumed.item_ids,
        result.consumed.currency_ids,
    )
    .await?;
    ctx.send_reply(CmdId::Act243UpgradeGiftCmd, result.reply, 0, req.up_tag)
        .await
}

pub async fn on_act243_unlock_racer(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act243UnlockRacerRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let result = ctx
        .player()?
        .activity
        .unlock_act243_racer(
            db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.racer_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        result.consumed.item_ids,
        result.consumed.currency_ids,
    )
    .await?;
    ctx.send_reply(CmdId::Act243UnlockRacerCmd, result.reply, 0, req.up_tag)
        .await
}
