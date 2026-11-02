use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    ChangeMapRequest, CmdId, ExploreInteractReply, ExploreInteractRequest, ExploreItemChangePush,
    ExploreItemInteractReply, ExploreItemInteractRequest,
    ExploreMoveReply, ExploreMoveRequest, ExploreStep, ExploreStepPush, ExploreUseItemReply,
    ExploreUseItemRequest, GetExploreInfoRequest, MarkPopDeepSettleRequest, MarkPopShallowSettleRequest,
    UpdateOpenPush, ResetExploreReply, ResetExploreRequest,
};

fn explore_error(error: anyhow::Error) -> AppError {
    tracing::warn!(error = %error, "exploration request rejected");
    if error.chain().any(|cause| cause.downcast_ref::<sqlx::Error>().is_some()) {
        AppError::Custom(error.to_string())
    } else {
        AppError::InvalidRequest
    }
}

async fn send_explore_steps(ctx: &mut ConnectionContext, steps: Vec<ExploreStep>) -> Result<(), AppError> {
    if !steps.is_empty() {
        ctx.notify(CmdId::ExploreStepPushCmd, ExploreStepPush { steps }).await?;
    }
    Ok(())
}

pub async fn on_change_map(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = ChangeMapRequest::decode(&req.data[..])?;
    let reply = logic::explore_entry::change_map(
        ctx.state.db, player_id, request.map_id.ok_or(AppError::InvalidRequest)?,
    ).await.map_err(explore_error)?;
    ctx.send_reply(CmdId::ChangeMapCmd, reply, 0, req.up_tag).await
}

pub async fn on_get_explore_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    GetExploreInfoRequest::decode(&req.data[..])?;
    let reply = logic::explore_entry::current_info(ctx.state.db, ctx.player()?.id)
        .await.map_err(explore_error)?;
    ctx.send_reply(CmdId::GetExploreInfoCmd, reply, 0, req.up_tag).await
}

pub async fn on_explore_move(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ExploreMoveRequest::decode(&req.data[..])?;
    if let Some(id) = request.interact_id.filter(|id| *id != 0) {
        let steps = logic::explore_light::move_prism(ctx.state.db, ctx.player()?.id, id,
            request.posx.ok_or(AppError::InvalidRequest)?, request.posy.ok_or(AppError::InvalidRequest)?)
            .await.map_err(explore_error)?;
        send_explore_steps(ctx, steps).await?;
        return ctx.send_reply(CmdId::ExploreMoveCmd, ExploreMoveReply {}, 0, req.up_tag).await;
    }
    let movement = logic::explore_move::move_player(
        ctx.state.db, ctx.player()?.id,
        request.posx.ok_or(AppError::InvalidRequest)?, request.posy.ok_or(AppError::InvalidRequest)?,
    ).await.map_err(explore_error)?;
    ctx.player_mut()?.tasks.record_updates(&movement.tasks);
    crate::util::task_events::notify_tasks(ctx, movement.tasks).await?;
    send_explore_steps(ctx, movement.steps).await?;
    ctx.send_reply(CmdId::ExploreMoveCmd, ExploreMoveReply {}, 0, req.up_tag).await
}

pub async fn on_explore_interact(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ExploreInteractRequest::decode(&req.data[..])?;
    let object_id = request.id.ok_or(AppError::InvalidRequest)?;
    let player_id = ctx.player()?.id;
    let kind = logic::explore_engine::current_object_kind(ctx.state.db, player_id, object_id)
        .await.map_err(explore_error)?;
    let steps = match kind {
        3 => match logic::explore_battle::interact(ctx.state.db, player_id, object_id)
            .await.map_err(explore_error)? {
                logic::explore_battle::BattleInteraction::Begin { step, .. } => vec![step],
                logic::explore_battle::BattleInteraction::Finish { steps } => steps,
            },
        228 => logic::explore_engine::collect_basic_archive(ctx.state.db, player_id, object_id)
            .await.map_err(explore_error)?.1,
        1 | 232 => {
            let claim = logic::explore_chest::claim(ctx.state.db, player_id, object_id)
                .await.map_err(explore_error)?;
            crate::util::push::send_applied_reward_pushes(
                ctx, player_id, claim.rewards, claim.material_changes,
                Some(crate::types::material_get_approach::MaterialGetApproach::Explore),
            ).await?;
            if !claim.items.is_empty() {
                ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: claim.items }).await?;
            }
            claim.steps
        }
        212 => {
            let swap = logic::explore_rune::swap(ctx.state.db, player_id, object_id)
                .await.map_err(explore_error)?;
            ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: vec![swap.item] }).await?;
            swap.steps
        }
        201 => logic::explore_spike::trigger(ctx.state.db, player_id, object_id)
            .await.map_err(explore_error)?,
        218 | 219 => logic::explore_light::rotate(ctx.state.db, player_id, object_id,
            request.params.as_deref().ok_or(AppError::InvalidRequest)?)
            .await.map_err(explore_error)?,
        223 => logic::explore_pipe::rotate(ctx.state.db, player_id, object_id,
            request.params.as_deref().ok_or(AppError::InvalidRequest)?)
            .await.map_err(explore_error)?,
        222 => {
            if !request.params.as_deref().unwrap_or("").is_empty() { return Err(AppError::InvalidRequest); }
            logic::explore_pipe::interact_entrance(ctx.state.db, player_id, object_id)
                .await.map_err(explore_error)?
        }
        224 => logic::explore_pipe::interact_pot(ctx.state.db, player_id, object_id, request.params.as_deref())
            .await.map_err(explore_error)?,
        229 => logic::explore_bonus::finish(ctx.state.db, player_id, object_id,
            request.params.as_deref().ok_or(AppError::InvalidRequest)?)
            .await.map_err(explore_error)?,
        213 => logic::explore_teleport::transfer(ctx.state.db, player_id, object_id)
            .await.map_err(explore_error)?,
        217 => {
            let change = logic::explore_rock::take(ctx.state.db, player_id, object_id)
                .await.map_err(explore_error)?;
            ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: vec![change.item] }).await?;
            change.steps
        }
        214 => {
            let result = logic::explore_exit::complete(ctx.state.db, player_id, object_id)
                .await.map_err(explore_error)?;
            ctx.notify(CmdId::DungeonUpdatePushCmd, result.dungeon_update).await?;
            if !result.open_infos.is_empty() {
                ctx.notify(CmdId::UpdateOpenPushCmd, UpdateOpenPush { open_infos: result.open_infos }).await?;
            }
            crate::util::task_events::notify(
                ctx, player_id, logic::task::TaskEvent::EpisodeFinish { episode_id: result.episode_id },
            ).await?;
            result.steps
        }
        _ => return Err(AppError::InvalidRequest),
    };
    send_explore_steps(ctx, steps).await?;
    ctx.send_reply(CmdId::ExploreInteractCmd, ExploreInteractReply { id: Some(object_id), params: request.params }, 0, req.up_tag).await
}

pub async fn on_explore_use_item(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ExploreUseItemRequest::decode(&req.data[..])?;
    let uid = request.uid.ok_or(AppError::InvalidRequest)?;
    let player_id = ctx.player()?.id;
    let effect = logic::explore_rock::item_effect(ctx.state.db, player_id, uid).await.map_err(explore_error)?;
    let steps = match effect.as_str() {
        "2" if request.posx.unwrap_or(0) == 0 && request.posy.unwrap_or(0) == 0
            && request.interact_id.unwrap_or(0) == 0 => vec![
                logic::explore_rune::select(ctx.state.db, player_id, uid).await.map_err(explore_error)?
            ],
        "1#209" if request.interact_id.unwrap_or(0) == 0 => {
            let change = logic::explore_rock::place(ctx.state.db, player_id, uid,
                request.posx.ok_or(AppError::InvalidRequest)?, request.posy.ok_or(AppError::InvalidRequest)?)
                .await.map_err(explore_error)?;
            ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: vec![change.item] }).await?;
            change.steps
        }
        "3#216" if request.interact_id.unwrap_or(0) == 0 => {
            logic::explore_light::use_light_ball(ctx.state.db, player_id, uid,
                request.posx.ok_or(AppError::InvalidRequest)?, request.posy.ok_or(AppError::InvalidRequest)?)
                .await.map_err(explore_error)?
        }
        "4" if request.posx.unwrap_or(0) == 0 && request.posy.unwrap_or(0) == 0 => {
            let repair = logic::explore_light::repair_prism(ctx.state.db, player_id, uid,
                request.interact_id.filter(|id| *id > 0).ok_or(AppError::InvalidRequest)?)
                .await.map_err(explore_error)?;
            ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: vec![repair.item] }).await?;
            repair.steps
        }
        _ => return Err(AppError::InvalidRequest),
    };
    send_explore_steps(ctx, steps).await?;
    ctx.send_reply(CmdId::ExploreUseItemCmd, ExploreUseItemReply { uid: Some(uid) }, 0, req.up_tag).await
}

pub async fn on_explore_item_interact(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ExploreItemInteractRequest::decode(&req.data[..])?;
    if !request.params.as_deref().unwrap_or("").is_empty() { return Err(AppError::InvalidRequest); }
    let id = request.id.ok_or(AppError::InvalidRequest)?;
    let change = logic::explore_rock::pick_up(ctx.state.db, ctx.player()?.id, id).await.map_err(explore_error)?;
    ctx.notify(CmdId::ExploreItemChangePushCmd, ExploreItemChangePush { explore_items: vec![change.item] }).await?;
    send_explore_steps(ctx, change.steps).await?;
    ctx.send_reply(CmdId::ExploreItemInteractCmd, ExploreItemInteractReply { id: Some(id), params: request.params }, 0, req.up_tag).await
}

pub async fn on_reset_explore(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    ResetExploreRequest::decode(&req.data[..])?;
    let steps = logic::explore_reset::reset(ctx.state.db, ctx.player()?.id).await.map_err(explore_error)?;
    send_explore_steps(ctx, steps).await?;
    ctx.send_reply(CmdId::ResetExploreCmd, ResetExploreReply {}, 0, req.up_tag).await
}



pub async fn on_get_explore_simple_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx.player()?.exploration.simple_info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetExploreSimpleInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_weekwalk_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx
        .player()?
        .exploration
        .weekwalk_info(ctx.state.db)
        .await?;
    ctx.send_reply(CmdId::GetWeekwalkInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_pop_shallow_settle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    MarkPopShallowSettleRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .mark_pop_shallow_settle(ctx.state.db)
        .await?;
    ctx.send_reply(CmdId::MarkPopShallowSettleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_pop_deep_settle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    MarkPopDeepSettleRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .mark_pop_deep_settle(ctx.state.db)
        .await?;
    ctx.send_reply(CmdId::MarkPopDeepSettleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_weekwalk_ver2_get_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx
        .player()?
        .exploration
        .weekwalk_v2_info(ctx.state.db)
        .await?;
    ctx.send_reply(CmdId::WeekwalkVer2GetInfoCmd, reply, 0, req.up_tag)
        .await
}
