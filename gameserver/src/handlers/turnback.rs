use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::{material_get_approach::MaterialGetApproach, red_dot_id::RedDotId},
    util::push,
};
use prost::Message;
use sonettobuf::{
    AcceptAllTurnbackBonusPointRequest, AcceptAllTurnbackReturnRewardRequest,
    BuyDoubleBonusRequest, CmdId, GetTurnbackDailyBonusRequest, GetTurnbackInfoRequest,
    TurnbackBonusPointRequest, TurnbackFirstShowRequest, TurnbackOnceBonusRequest,
    TurnbackReturnRewardRequest, TurnbackSignInRequest,
};
pub async fn on_get_turnback_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    GetTurnbackInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .turnback
        .info(ctx.state.db, ctx.state.tables)
        .await?;
    ctx.send_reply(CmdId::GetTurnbackInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_turnback_first_show(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = TurnbackFirstShowRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .turnback
        .mark_first_show(ctx.state.db, msg.id.unwrap_or_default())
        .await?;
    ctx.send_reply(CmdId::TurnbackFirstShowCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_turnback_once_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TurnbackOnceBonusRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_once_bonus(ctx.state.db, msg.id.unwrap_or_default())
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    push::clear_red_dots(ctx, [RedDotId::TurnbackOnceBonus.id()]).await?;
    ctx.send_reply(CmdId::TurnbackOnceBonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_turnback_sign_in(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TurnbackSignInRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_sign_in(
            ctx.state.db,
            msg.id.unwrap_or_default(),
            msg.day.unwrap_or_default(),
        )
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    push::clear_red_dots(ctx, [RedDotId::TurnbackSignIn.id()]).await?;
    ctx.send_reply(CmdId::TurnbackSignInCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_turnback_daily_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = GetTurnbackDailyBonusRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_daily_bonus(ctx.state.db, msg.id.unwrap_or_default())
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    push::clear_red_dots(ctx, [RedDotId::TurnbackDailyBonus.id()]).await?;
    ctx.send_reply(CmdId::GetTurnbackDailyBonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_turnback_bonus_point(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TurnbackBonusPointRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_bonus_point(
            ctx.state.db,
            msg.id.unwrap_or_default(),
            msg.bonus_point_id.unwrap_or_default(),
            ctx.state.tables,
        )
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(CmdId::TurnbackBonusPointCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_accept_all_turnback_bonus_point(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = AcceptAllTurnbackBonusPointRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_all_bonus_points(ctx.state.db, msg.id.unwrap_or_default(), ctx.state.tables)
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    push::clear_red_dots(
        ctx,
        [
            RedDotId::TurnbackLegacyTask.id(),
            RedDotId::TurnbackBattlePassBonus.id(),
            RedDotId::TurnbackBattlePassTask.id(),
        ],
    )
    .await?;
    ctx.send_reply(
        CmdId::AcceptAllTurnbackBonusPointCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_turnback_return_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TurnbackReturnRewardRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_return_reward(
            ctx.state.db,
            msg.id.ok_or(AppError::InvalidRequest)?,
            msg.reward_id.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(CmdId::TurnbackReturnRewardCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_accept_all_turnback_return_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = AcceptAllTurnbackReturnRewardRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .claim_all_return_rewards(
            ctx.state.db,
            msg.id.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(
        CmdId::AcceptAllTurnbackReturnRewardCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_buy_double_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = BuyDoubleBonusRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .turnback
        .buy_double_bonus(ctx.state.db, msg.id.unwrap_or_default())
        .await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(CmdId::BuyDoubleBonusCmd, claim.reply, 0, req.up_tag)
        .await
}
