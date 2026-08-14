use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{
    BpBuyLevelRequset, BpMarkFirstShowRequest, CmdId, GetAct233BpBonusRequest,
    GetAct233BpInfoRequest, GetBpBonusRequest, GetBpInfoRequest, GetSelfSelectBonusRequest,
};
use std::{future::Future, pin::Pin};

pub async fn on_get_act233_bp_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct233BpInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .battle_pass
        .act233_info(ctx.state.db, msg.activity_id, msg.get_task.unwrap_or(false))
        .await?;
    ctx.send_reply(CmdId::GetAct233BpInfoCmd, reply, 0, req.up_tag)
        .await
}

pub fn on_get_act233_bp_bonus<'a>(
    ctx: &'a mut ConnectionContext,
    req: ClientPacket,
) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
    Box::pin(async move {
        let player_id = ctx.player()?.id;
        let msg = GetAct233BpBonusRequest::decode(&req.data[..])?;
        let activity_id = msg.activity_id.ok_or(AppError::InvalidRequest)?;
        let claim = ctx
            .player()?
            .battle_pass
            .claim_act233_bonus(ctx.state.db, Some(activity_id), msg.level, msg.pay_bonus)
            .await?;

        push::send_applied_reward_pushes(
            ctx,
            player_id,
            claim.rewards,
            claim.material_changes,
            Some(MaterialGetApproach::ActBp),
        )
        .await?;
        let red_dot_groups = ctx
            .player()?
            .red_dot
            .act233_groups(ctx.state.db, activity_id)
            .await?;
        push::send_red_dot_groups(ctx, red_dot_groups).await?;
        ctx.send_reply(CmdId::GetAct233BpBonusCmd, claim.reply, 0, req.up_tag)
            .await
    })
}

pub async fn on_get_bp_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetBpInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .battle_pass
        .info(ctx.state.db, msg.get_task.unwrap_or(false))
        .await?;
    ctx.send_reply(CmdId::GetBpInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_bp_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = GetBpBonusRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .battle_pass
        .claim_bonus(ctx.state.db, msg.id, msg.level, msg.pay_bonus, msg.is_sp)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::BattlePass),
    )
    .await?;
    let red_dot_groups = ctx
        .player()?
        .red_dot
        .battle_pass_groups(ctx.state.db)
        .await?;
    push::send_red_dot_groups(ctx, red_dot_groups).await?;
    ctx.send_reply(CmdId::GetBpBonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_self_select_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = GetSelfSelectBonusRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .battle_pass
        .claim_self_select_bonus(ctx.state.db, msg.id, msg.level, msg.index)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::BattlePass),
    )
    .await?;
    let red_dot_groups = ctx
        .player()?
        .red_dot
        .battle_pass_groups(ctx.state.db)
        .await?;
    push::send_red_dot_groups(ctx, red_dot_groups).await?;
    ctx.send_reply(CmdId::GetSelfSelectBonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_buy_level(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = BpBuyLevelRequset::decode(&req.data[..])?;
    let purchase = ctx
        .player()?
        .battle_pass
        .buy_levels(ctx.state.db, msg.id, msg.num)
        .await?;

    push::send_currency_change_push(ctx, player_id, vec![purchase.currency_change]).await?;
    push::send_material_change_push(
        ctx,
        vec![purchase.material_change],
        Some(MaterialGetApproach::BattlePass),
    )
    .await?;
    let red_dot_groups = ctx
        .player()?
        .red_dot
        .battle_pass_groups(ctx.state.db)
        .await?;
    push::send_red_dot_groups(ctx, red_dot_groups).await?;
    ctx.send_reply(CmdId::BpBuyLevelRequsetCmd, purchase.reply, 0, req.up_tag)
        .await
}

pub async fn on_bp_mark_first_show(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = BpMarkFirstShowRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .battle_pass
        .mark_first_show(ctx.state.db, msg.id, msg.is_sp)
        .await?;

    ctx.send_reply(CmdId::BpMarkFirstShowCmd, reply, 0, req.up_tag)
        .await
}
