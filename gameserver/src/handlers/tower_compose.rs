use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    tower_compose,
};
use prost::Message;
use sonettobuf::{CmdId, TowerComposeGetInfoRequest, TowerComposeSetModsRequest,
    TowerComposeUpdateRecordRequest, TowerComposeLoadRecordRequest,
    TowerComposeReChallengeRequest, TowerComposeCancelReChallengeRequest};

pub async fn on_tower_compose_get_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    TowerComposeGetInfoRequest::decode(&req.data[..])?;
    let reply = tower_compose::get_info(ctx.state.db, player_id).await?;

    ctx.send_reply(CmdId::TowerComposeGetInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_tower_compose_set_mods(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TowerComposeSetModsRequest::decode(&req.data[..])?;
    let reply = tower_compose::set_mods(
        ctx.state.db,
        player_id,
        msg.theme_id.unwrap_or_default(),
        msg.plane_mods,
    )
    .await?;

    ctx.send_reply(CmdId::TowerComposeSetModsCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_record(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TowerComposeUpdateRecordRequest::decode(&req.data[..])?;
    let reply = tower_compose::update_record(ctx.state.db, player_id, msg.theme_id.ok_or(AppError::InvalidRequest)?, msg.update.unwrap_or(false)).await?;
    ctx.send_reply(CmdId::TowerComposeUpdateRecordCmd, reply, 0, req.up_tag).await
}

pub async fn on_load_record(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TowerComposeLoadRecordRequest::decode(&req.data[..])?;
    let reply = tower_compose::load_record(ctx.state.db, player_id, msg.theme_id.ok_or(AppError::InvalidRequest)?).await?;
    ctx.send_reply(CmdId::TowerComposeLoadRecordCmd, reply, 0, req.up_tag).await
}

pub async fn on_re_challenge(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    ctx.player()?.battle.ensure_can_start(ctx.state.db, player_id).await?;
    let msg = TowerComposeReChallengeRequest::decode(&req.data[..])?;
    let reply = tower_compose::re_challenge(ctx.state.db, player_id, msg.theme_id.ok_or(AppError::InvalidRequest)?, msg.plane_id.ok_or(AppError::InvalidRequest)?).await?;
    ctx.send_reply(CmdId::TowerComposeReChallengeCmd, reply, 0, req.up_tag).await
}

pub async fn on_cancel_re_challenge(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    ctx.player()?.battle.ensure_can_start(ctx.state.db, player_id).await?;
    let msg = TowerComposeCancelReChallengeRequest::decode(&req.data[..])?;
    let reply = tower_compose::cancel_re_challenge(ctx.state.db, player_id, msg.theme_id.ok_or(AppError::InvalidRequest)?).await?;
    ctx.send_reply(CmdId::TowerComposeCancelReChallengeCmd, reply, 0, req.up_tag).await
}
