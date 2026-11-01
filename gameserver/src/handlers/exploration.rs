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
    if std::env::var_os("ENIGMA_TRACE_WEEKWALK").is_some() {
        tracing::info!(target: "weekwalk_trace", command = "GetWeekwalkInfoCmd",
            result = "reply_prepared", reply_bytes = reply.encoded_len(),
            info_present = reply.info.is_some(), time_this_week = ?reply.time_this_week,
            "weekwalk response summary");
        if let Some(info) = &reply.info {
            tracing::info!(target: "weekwalk_trace", time = ?info.time, end_time = ?info.end_time,
                issue_id = ?info.issue_id, max_layer = ?info.max_layer,
                is_open_deep = ?info.is_open_deep, map_count = info.map_info.len(),
                is_pop_shallow_settle = ?info.is_pop_shallow_settle,
                is_pop_deep_settle = ?info.is_pop_deep_settle,
                "weekwalk state fields");
            for map in info.map_info.iter().take(32) {
                tracing::info!(target: "weekwalk_trace", map_id = ?map.id, scene_id = ?map.scene_id,
                    is_finish = ?map.is_finish, is_finished = ?map.is_finished,
                    element_count = map.element_infos.len(), battle_count = map.battle_infos.len(),
                    hero_count = map.hero_infos.len(), story_count = map.story_ids.len(),
                    "weekwalk map fields");
                for element in map.element_infos.iter().take(32) {
                    tracing::info!(target: "weekwalk_trace", map_id = ?map.id,
                        element_id = ?element.element_id, index = ?element.index,
                        visible = ?element.visible, is_finish = ?element.is_finish,
                        history_count = element.historylist.len(), "weekwalk element fields");
                }
            }
        }
    }
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

pub async fn on_weekwalk_dialog_history(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::WeekwalkDialogHistoryRequest::decode(&req.data[..])?;
    let map_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    let element_id = message.element_id.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk::save_dialog_history(
        ctx.state.db, ctx.player()?.id, map_id, element_id, &message.historylist,
    ).await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::WeekwalkDialogHistoryCmd, sonettobuf::WeekwalkDialogHistoryReply {
        element_id: Some(element_id), layer_id: Some(map_id), historylist: message.historylist,
    }, 0, req.up_tag).await
}

pub async fn on_weekwalk_dialog(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let msg = sonettobuf::WeekwalkDialogRequest::decode(&req.data[..])?;
    let player_id = ctx.player()?.id;
    let layer = msg.layer_id.ok_or(AppError::InvalidRequest)?;
    let element = msg.element_id.ok_or(AppError::InvalidRequest)?;
    let option = msg.option.unwrap_or_default();
    if !database::db::game::weekwalk::advance_opening_dialog(ctx.state.db, player_id, layer, element, option)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::WeekwalkDialogCmd, sonettobuf::WeekwalkDialogReply {
        element_id: Some(element), layer_id: Some(layer), option: Some(option),
    }, 0, req.up_tag).await?;
    push_weekwalk_info(ctx, player_id).await
}

pub async fn push_weekwalk_info(ctx: &mut ConnectionContext, player_id: i64) -> Result<(), AppError> {
    let info = logic::exploration::ExplorationManager::new(player_id).weekwalk_info(ctx.state.db).await?.info;
    ctx.notify(CmdId::WeekwalkInfoUpdatePushCmd, sonettobuf::WeekwalkInfoUpdatePush { info }).await
}

pub async fn on_before_weekwalk_battle(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let msg = sonettobuf::BeforeStartWeekwalkBattleRequest::decode(&req.data[..])?;
    let layer = msg.layer_id.ok_or(AppError::InvalidRequest)?;
    let element = msg.element_id.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk::opening_battle_available(ctx.state.db, ctx.player()?.id, layer, element)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::BeforeStartWeekwalkBattleCmd, sonettobuf::BeforeStartWeekwalkBattleReply {
        element_id: Some(element), layer_id: Some(layer),
    }, 0, req.up_tag).await
}

pub async fn on_change_weekwalk_team(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::ChangeWeekwalkHeroGroupSelectRequest::decode(&req.data[..])?;
    let map_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    let battle_id = message.battle_id.ok_or(AppError::InvalidRequest)?;
    let select = message.select.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk::set_battle_team(ctx.state.db, ctx.player()?.id, map_id, battle_id, select)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::ChangeWeekwalkHeroGroupSelectCmd, sonettobuf::ChangeWeekwalkHeroGroupSelectReply {
        layer_id: Some(map_id), battle_id: Some(battle_id), select: Some(select),
    }, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_show_buff(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::MarkShowBuffRequest::decode(&req.data[..])?;
    let layer_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk::clear_map_notice(ctx.state.db, ctx.player()?.id, layer_id, true)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::MarkShowBuffCmd, sonettobuf::MarkShowBuffReply { layer_id: Some(layer_id) }, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_show_finished(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::MarkShowFinishedRequest::decode(&req.data[..])?;
    let layer_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk::clear_map_notice(ctx.state.db, ctx.player()?.id, layer_id, false)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::MarkShowFinishedCmd, sonettobuf::MarkShowFinishedReply { layer_id: Some(layer_id) }, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_deep_rule(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    sonettobuf::MarkPopDeepRuleRequest::decode(&req.data[..])?;
    database::db::game::weekwalk::clear_deep_rule_notice(ctx.state.db, ctx.player()?.id).await.map_err(explore_error)?;
    ctx.send_reply(CmdId::MarkPopDeepRuleCmd, sonettobuf::MarkPopDeepRuleReply {}, 0, req.up_tag).await
}

pub async fn on_change_weekwalk_v2_team(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::WeekwalkVer2ChangeHeroGroupSelectRequest::decode(&req.data[..])?;
    let layer_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    let battle_id = message.battle_id.ok_or(AppError::InvalidRequest)?;
    let select = message.select.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk_v2::set_battle_team(ctx.state.db, ctx.player()?.id, layer_id, battle_id, select)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::WeekwalkVer2ChangeHeroGroupSelectCmd, sonettobuf::WeekwalkVer2ChangeHeroGroupSelectReply {
        layer_id: Some(layer_id), battle_id: Some(battle_id), select: Some(select),
    }, 0, req.up_tag).await
}

pub async fn on_choose_weekwalk_v2_skill(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::WeekwalkVer2ChooseSkillRequest::decode(&req.data[..])?;
    let no = message.no.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk_v2::choose_team_skill(ctx.state.db, ctx.player()?.id, no, &message.skill_ids)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::WeekwalkVer2ChooseSkillCmd, sonettobuf::WeekwalkVer2ChooseSkillReply {
        no: Some(no), skill_ids: message.skill_ids,
    }, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_v2_show_finished(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let message = sonettobuf::WeekwalkVer2MarkShowFinishedRequest::decode(&req.data[..])?;
    let layer_id = message.layer_id.ok_or(AppError::InvalidRequest)?;
    if !database::db::game::weekwalk_v2::clear_layer_finished_notice(ctx.state.db, ctx.player()?.id, layer_id)
        .await.map_err(explore_error)? { return Err(AppError::InvalidRequest); }
    ctx.send_reply(CmdId::WeekwalkVer2MarkShowFinishedCmd, sonettobuf::WeekwalkVer2MarkShowFinishedReply {
        layer_id: Some(layer_id),
    }, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_v2_rule(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    sonettobuf::WeekwalkVer2MarkPopRuleRequest::decode(&req.data[..])?;
    database::db::game::weekwalk_v2::clear_rule_notice(ctx.state.db, ctx.player()?.id).await.map_err(explore_error)?;
    ctx.send_reply(CmdId::WeekwalkVer2MarkPopRuleCmd, sonettobuf::WeekwalkVer2MarkPopRuleReply {}, 0, req.up_tag).await
}

pub async fn on_mark_weekwalk_v2_previous_settlement(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    sonettobuf::WeekwalkVer2MarkPreSettleRequest::decode(&req.data[..])?;
    database::db::game::weekwalk_v2::clear_previous_settlement_notice(ctx.state.db, ctx.player()?.id).await.map_err(explore_error)?;
    ctx.send_reply(CmdId::WeekwalkVer2MarkPreSettleCmd, sonettobuf::WeekwalkVer2MarkPreSettleReply {}, 0, req.up_tag).await
}
