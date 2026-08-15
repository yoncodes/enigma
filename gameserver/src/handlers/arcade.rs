use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{
    ArcadeAttrChangePush, ArcadeGainRewardRequest, ArcadeGetOutSideInfoRequest,
    ArcadePlayerMoveRequest, ArcadeSwitchCharacterRequest, ArcadeTalentUpgradeRequest, CmdId,
};

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
