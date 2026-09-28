use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::push,
};
use prost::Message;
use sonettobuf::{BuildTrapRequest, CmdId, PutTrapRequest, UpgradeElementRequest};

pub async fn on_upgrade_element(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UpgradeElementRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .activity
        .upgrade_act116_element(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.element_id.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::UpgradeElementCmd, update.reply, 0, req.up_tag)
        .await?;
    push::send_cost_pushes(ctx, player_id, update.item_ids, update.currency_ids).await
}

pub async fn on_build_trap(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = BuildTrapRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .activity
        .build_act116_trap(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.trap_id.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::BuildTrapCmd, update.reply, 0, req.up_tag)
        .await?;
    push::send_cost_pushes(ctx, player_id, update.item_ids, update.currency_ids).await
}

pub async fn on_put_trap(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let msg = PutTrapRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .put_act116_trap(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.trap_id.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::PutTrapCmd, reply, 0, req.up_tag)
        .await
}
