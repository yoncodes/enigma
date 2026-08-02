use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::{material_get_approach::MaterialGetApproach, red_dot_id::RedDotId},
    util::push,
};
use prost::Message;
use sonettobuf::{
    CmdId, Get101BonusListRequest, Get101BonusRequest, Get101InfosRequest, Get101SpBonusRequest,
};

pub async fn on_get_101_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get101InfosRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .get101_infos(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::Get101InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_101_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Get101BonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get101_bonus(db, msg.activity_id, msg.id)
        .await?;

    if let Some(rewards) = &claim.rewards {
        push::send_item_change_push(
            ctx,
            player_id,
            rewards.item_ids.clone(),
            rewards.power_item_ids.clone(),
            rewards.insight_item_ids.clone(),
        )
        .await?;
        push::send_currency_change_push(ctx, player_id, rewards.currency_ids.clone()).await?;
        push::send_equip_update_push(ctx, player_id, rewards.equip_uids.clone()).await?;
        push::send_hero_update_push(ctx, player_id, rewards.hero_ids.clone()).await?;
        push::send_skin_gain_pushes(
            ctx,
            &rewards.skin_gains,
            Some(MaterialGetApproach::Activity),
        )
        .await?;
        push::send_bp_score_update_pushes(ctx, &rewards.bp_scores).await?;
        push::send_material_change_push(
            ctx,
            claim.material_changes.clone(),
            Some(MaterialGetApproach::Activity),
        )
        .await?;
    }

    let activity_id = claim.reply.activity_id.unwrap_or_default();
    push::send_red_dot_value_push(
        ctx,
        RedDotId::ActivityNoviceTab.id(),
        vec![activity_id],
        false,
        i32::from(claim.has_claimable),
        0,
    )
    .await?;

    ctx.send_reply(CmdId::Get101BonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_101_bonus_list(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Get101BonusListRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get101_bonus_list(db, msg.activity_id, msg.ids)
        .await?;

    if let Some(rewards) = claim.rewards {
        push::send_applied_reward_pushes(
            ctx,
            player_id,
            rewards,
            claim.material_changes,
            Some(MaterialGetApproach::Activity),
        )
        .await?;
    }

    let activity_id = claim.reply.activity_id.unwrap_or_default();
    push::send_red_dot_value_push(
        ctx,
        RedDotId::ActivityNoviceTab.id(),
        vec![activity_id],
        false,
        i32::from(claim.has_claimable),
        0,
    )
    .await?;

    ctx.send_reply(CmdId::Get101BonusListCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_101_sp_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Get101SpBonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get101_sp_bonus(db, msg.activity_id, msg.id)
        .await?;

    push::send_item_change_push(
        ctx,
        player_id,
        claim.rewards.item_ids.clone(),
        claim.rewards.power_item_ids.clone(),
        claim.rewards.insight_item_ids.clone(),
    )
    .await?;
    push::send_currency_change_push(ctx, player_id, claim.rewards.currency_ids.clone()).await?;
    push::send_equip_update_push(ctx, player_id, claim.rewards.equip_uids.clone()).await?;
    push::send_hero_update_push(ctx, player_id, claim.rewards.hero_ids.clone()).await?;
    push::send_skin_gain_pushes(
        ctx,
        &claim.rewards.skin_gains,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    push::send_bp_score_update_pushes(ctx, &claim.rewards.bp_scores).await?;
    push::send_material_change_push(
        ctx,
        claim.material_changes.clone(),
        Some(MaterialGetApproach::Activity),
    )
    .await?;

    let activity_id = claim.reply.activity_id.unwrap_or_default();
    push::send_red_dot_value_push(
        ctx,
        RedDotId::ActivityNoviceTab.id(),
        vec![activity_id],
        false,
        i32::from(claim.has_claimable),
        0,
    )
    .await?;

    ctx.send_reply(CmdId::Get101SpBonusCmd, claim.reply, 0, req.up_tag)
        .await
}
