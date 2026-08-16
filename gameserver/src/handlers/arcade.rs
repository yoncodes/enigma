use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::{push, task_events},
};
use prost::Message;
use sonettobuf::{
    ArcadeAttrChangePush, ArcadeGainRewardRequest, ArcadeGetInSideInfoRequest,
    ArcadeGetOutSideInfoRequest, ArcadePlayerMoveRequest, ArcadeSaveGameRequest,
    ArcadeSettleGameRequest, ArcadeSwitchCharacterRequest, ArcadeTalentUpgradeRequest, CmdId,
};
use std::{future::Future, pin::Pin};

pub fn on_get_inside_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
    Box::pin(get_inside_info(ctx, req))
}

async fn get_inside_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    ArcadeGetInSideInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .arcade
        .inside_info(ctx.state.db, ctx.state.tables)
        .await?;
    ctx.send_reply(CmdId::ArcadeGetInSideInfoCmd, reply, 0, req.up_tag)
        .await
}

pub fn on_save_game(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
    Box::pin(save_game(ctx, req))
}

async fn save_game(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ArcadeSaveGameRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .arcade
        .save_inside(
            ctx.state.db,
            ctx.state.tables,
            request.info.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::ArcadeSaveGameCmd, reply, 0, req.up_tag)
        .await
}

pub fn on_settle_game(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
    Box::pin(settle_game(ctx, req))
}

async fn settle_game(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ArcadeSettleGameRequest::decode(&req.data[..])?;
    let settlement = ctx
        .player()?
        .arcade
        .settle_inside(
            ctx.state.db,
            ctx.state.tables,
            request.r#type.ok_or(AppError::InvalidRequest)?,
            request.info.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.player_mut()?.tasks.record_updates(&settlement.tasks);
    task_events::notify_tasks(ctx, settlement.tasks).await?;
    ctx.push_red_dot_value(settlement.red_dot_id, vec![0], true, 1, 0)
        .await?;
    ctx.notify(
        CmdId::ArcadeAttrChangePushCmd,
        ArcadeAttrChangePush {
            attr: vec![settlement.changed_attr],
        },
    )
    .await?;
    ctx.send_reply(CmdId::ArcadeSettleGameCmd, settlement.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_outside_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    ArcadeGetOutSideInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .arcade
        .info(ctx.state.db, ctx.state.tables)
        .await?;
    ctx.send_reply(CmdId::ArcadeGetOutSideInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_player_move(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = ArcadePlayerMoveRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .arcade
        .move_player(
            ctx.state.db,
            ctx.state.tables,
            request.x.ok_or(AppError::InvalidRequest)?,
            request.y.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::ArcadePlayerMoveCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_switch_character(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = ArcadeSwitchCharacterRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .arcade
        .switch_character(
            ctx.state.db,
            ctx.state.tables,
            request.character_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::ArcadeSwitchCharacterCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_talent_upgrade(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = ArcadeTalentUpgradeRequest::decode(&req.data[..])?;
    let upgrade = ctx
        .player()?
        .arcade
        .upgrade_talent(
            ctx.state.db,
            ctx.state.tables,
            request.talent_id.ok_or(AppError::InvalidRequest)?,
            request.level.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.notify(
        CmdId::ArcadeAttrChangePushCmd,
        ArcadeAttrChangePush {
            attr: vec![upgrade.changed_attr],
        },
    )
    .await?;
    ctx.send_reply(CmdId::ArcadeTalentUpgradeCmd, upgrade.reply, 0, req.up_tag)
        .await
}

pub async fn on_gain_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = ArcadeGainRewardRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .arcade
        .gain_rewards(
            ctx.state.db,
            ctx.state.tables,
            request.reward_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::ArcadeReward),
    )
    .await?;
    ctx.push_red_dot(claim.red_dot_id, vec![0], true).await?;
    ctx.send_reply(CmdId::ArcadeGainRewardCmd, claim.reply, 0, req.up_tag)
        .await
}
