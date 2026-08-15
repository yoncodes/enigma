use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::{push, task_events},
};
use prost::Message;
use sonettobuf::{
    CmdId, GainFinalInviteRewardRequest, GainInviteRewardRequest, GetHeroInvitationInfoRequest,
};

pub async fn on_get_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    GetHeroInvitationInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .hero_invitation
        .info(ctx.state.db, ctx.state.tables)
        .await?;

    ctx.send_reply(CmdId::GetHeroInvitationInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_gain_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = GainInviteRewardRequest::decode(&req.data[..])?;
    let invite_id = request.id.ok_or(AppError::InvalidRequest)?;
    let db = ctx.state.db;
    let tables = ctx.state.tables;
    let claim = ctx
        .player_mut()?
        .hero_invitation
        .gain_reward(db, tables, invite_id)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.player_mut()?.tasks.record_updates(&claim.updated_tasks);
    task_events::notify_tasks(ctx, claim.updated_tasks).await?;

    ctx.send_reply(CmdId::GainInviteRewardCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_gain_final_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    GainFinalInviteRewardRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let tables = ctx.state.tables;
    let claim = ctx
        .player_mut()?
        .hero_invitation
        .gain_final_reward(db, tables)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;

    ctx.send_reply(CmdId::GainFinalInviteRewardCmd, claim.reply, 0, req.up_tag)
        .await
}
