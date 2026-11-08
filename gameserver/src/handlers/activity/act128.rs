use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::{material_get_approach::MaterialGetApproach, red_dot_id::RedDotId},
    util::push,
};
use prost::Message;
use sonettobuf::{
    Act128ChangeActModeRequest, Act128GetExpRequest, Act128GetMilestoneBonusRequest,
    Act128GetTotalRewardsRequest, CmdId, GetGalleryInfosRequest, ResetAct128TeamRequest,
    SetAct128TeamRequest,
};

pub async fn on_act128_change_act_mode(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Act128ChangeActModeRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .change_act128_mode(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.is_activity_mode_open.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::Act128ChangeActModeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_set_act128_team(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = SetAct128TeamRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .set_act128_team(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.boss_id.ok_or(AppError::InvalidRequest)?,
            msg.front_hero_slots,
            msg.back_hero_slots,
            msg.add_bond_group_id.unwrap_or_default(),
        )
        .await?;
    ctx.send_reply(CmdId::SetAct128TeamCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_reset_act128_team(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = ResetAct128TeamRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .reset_act128_team(
            ctx.state.db,
            msg.activity_id.ok_or(AppError::InvalidRequest)?,
            msg.boss_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::ResetAct128TeamCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_gallery_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetGalleryInfosRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .get_gallery_infos(ctx.state.db, msg.activity_id)
        .await?;
    ctx.send_reply(CmdId::GetGalleryInfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_act128_get_exp(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act128GetExpRequest::decode(&req.data[..])?;
    let claim = ctx
        .player()?
        .activity
        .get_act128_exp(ctx.state.db, msg.activity_id, msg.r#type)
        .await?;
    push::send_currency_change_push(ctx, player_id, claim.currency_changes).await?;
    ctx.send_reply(CmdId::Act128GetExpCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_act128_get_total_rewards(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act128GetTotalRewardsRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get_act128_total_rewards(db, msg.activity_id, msg.boss_id)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Act128BossReward),
    )
    .await?;
    push::send_red_dot_value_push(
        ctx,
        RedDotId::BossRushBossSchedule.id(),
        vec![0],
        true,
        0,
        0,
    )
    .await?;

    ctx.send_reply(CmdId::Act128GetTotalRewardsCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_act128_get_milestone_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act128GetMilestoneBonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get_act128_milestone_bonus(db, msg.activity_id)
        .await?;

    push::send_item_first_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Act128MilestoneBonus),
    )
    .await?;
    push::send_red_dot_value_push(ctx, RedDotId::BossRushRankBonus.id(), vec![0], true, 0, 0)
        .await?;

    ctx.send_reply(
        CmdId::Act128GetMilestoneBonusCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}

#[cfg(test)]
mod tests;
