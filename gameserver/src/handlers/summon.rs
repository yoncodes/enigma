use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::{push, task_events},
};
use logic::task::TaskEvent;
use prost::Message;
use sonettobuf::CmdId;
use sonettobuf::{
    ChooseEnhancedPoolHeroRequest, ChooseMultiUpHeroRequest, GetSummonProgressRewardsRequest,
    PopUpRecommendWindowRequest, SummonQueryTokenRequest, SummonRequest, UpdateGuidePush,
};

pub async fn on_get_summon_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx.player()?.summon.info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetSummonInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_summon_progress_rewards(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = GetSummonProgressRewardsRequest::decode(&req.data[..])?;
    let (reply, changed_items) = ctx
        .player()?
        .summon
        .progress_rewards(ctx.state.db, msg.pool_id.ok_or(AppError::InvalidRequest)?)
        .await?;

    push::send_item_change_push(ctx, player_id, changed_items, Vec::new(), Vec::new()).await?;
    ctx.send_reply(CmdId::GetSummonProgressRewardsCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_pop_up_recommend_window(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = PopUpRecommendWindowRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .summon
        .pop_up_recommend_window(
            ctx.state.db,
            msg.pool_id.ok_or(AppError::InvalidRequest)?,
            msg.order_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    ctx.send_reply(CmdId::PopUpRecommendWindowCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_summon_query_token(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    SummonQueryTokenRequest::decode(&req.data[..])?;
    let (reply, activity_push) = ctx.player()?.summon.query_token(ctx.state.db).await?;

    ctx.notify(CmdId::EndActivityPushCmd, activity_push).await?;
    ctx.send_reply(CmdId::SummonQueryTokenCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_summon(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = SummonRequest::decode(&req.data[..])?;
    let pool_id = msg.pool_id.ok_or(AppError::InvalidRequest)?;
    let count = msg.count.unwrap_or(1);
    let completion = ctx
        .player()?
        .summon
        .summon(ctx.state.db, pool_id, msg.guide_id, msg.step_id, count)
        .await?;
    let changed = completion.changed;

    push::send_item_change_push(
        ctx,
        player_id,
        changed.item_ids.clone(),
        changed.power_item_ids.clone(),
        changed.insight_item_ids.clone(),
    )
    .await?;
    push::send_currency_change_push(ctx, player_id, changed.currency_ids.clone()).await?;
    push::send_equip_update_push(ctx, player_id, changed.equip_uids.clone()).await?;
    push::send_hero_update_push(ctx, player_id, changed.hero_ids.clone()).await?;
    push::send_skin_gain_pushes(ctx, &changed.skin_gains, None).await?;
    push::send_bp_score_update_pushes(ctx, &changed.bp_scores).await?;
    if let Some(guide_info) = completion.guide_info {
        ctx.notify(
            CmdId::UpdateGuidePushCmd,
            UpdateGuidePush {
                guide_infos: vec![guide_info],
            },
        )
        .await?;
    }
    task_events::notify(ctx, player_id, TaskEvent::Summon { pool_id, count }).await?;
    ctx.send_reply(CmdId::SummonCmd, completion.reply, 0, req.up_tag)
        .await
}

pub async fn on_choose_enhanced_pool_hero(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = ChooseEnhancedPoolHeroRequest::decode(&req.data[..])?;
    let pool_id = msg.pool_id.ok_or(AppError::InvalidRequest)?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let reply = ctx
        .player()?
        .summon
        .choose_enhanced_pool_hero(ctx.state.db, pool_id, hero_id)
        .await?;

    ctx.send_reply(CmdId::ChooseEnhancedPoolHeroCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_choose_multi_up_hero(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = ChooseMultiUpHeroRequest::decode(&req.data[..])?;
    let pool_id = msg.pool_id.ok_or(AppError::InvalidRequest)?;
    let reply = ctx
        .player()?
        .summon
        .choose_multi_up_hero(ctx.state.db, pool_id, msg.hero_ids)
        .await?;

    ctx.send_reply(CmdId::ChooseMultiUpHeroCmd, reply, 0, req.up_tag)
        .await
}
