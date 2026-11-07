use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    BeforeStartWeekwalkBattleRequest, ChangeMapRequest, ChangeWeekwalkHeroGroupSelectRequest,
    CmdId, GetExploreInfoRequest, MarkPopDeepSettleRequest, MarkPopShallowSettleRequest,
    MarkShowFinishedRequest, WeekwalkDialogHistoryRequest, WeekwalkDialogRequest,
    WeekwalkVer2ChangeHeroGroupSelectRequest, WeekwalkVer2ChooseSkillRequest,
    WeekwalkVer2MarkShowFinishedRequest,
};

pub async fn on_change_map(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = ChangeMapRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .change_map(
            ctx.state.db,
            request.map_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::ChangeMapCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_explore_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    GetExploreInfoRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.exploration.info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetExploreInfoCmd, reply, 0, req.up_tag)
        .await
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
    let reply = ctx.player()?.weekwalk.info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetWeekwalkInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_before_start_weekwalk_battle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = BeforeStartWeekwalkBattleRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .weekwalk
        .select_battle(
            db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
            request.element_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::BeforeStartWeekwalkBattleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_change_weekwalk_hero_group_select(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = ChangeWeekwalkHeroGroupSelectRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .weekwalk
        .change_group_selection(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
            request.battle_id.ok_or(AppError::InvalidRequest)?,
            request.select.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(
        CmdId::ChangeWeekwalkHeroGroupSelectCmd,
        reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_weekwalk_dialog(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = WeekwalkDialogRequest::decode(&req.data[..])?;
    let (reply, update) = ctx
        .player()?
        .weekwalk
        .dialog(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
            request.element_id.ok_or(AppError::InvalidRequest)?,
            request.option.unwrap_or_default(),
        )
        .await?;
    ctx.send_reply(CmdId::WeekwalkDialogCmd, reply, 0, req.up_tag)
        .await?;
    ctx.notify(CmdId::WeekwalkInfoUpdatePushCmd, update).await
}

pub async fn on_weekwalk_dialog_history(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = WeekwalkDialogHistoryRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .weekwalk
        .store_dialog_history(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
            request.element_id.ok_or(AppError::InvalidRequest)?,
            request.historylist,
        )
        .await?;
    ctx.send_reply(CmdId::WeekwalkDialogHistoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_pop_shallow_settle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    MarkPopShallowSettleRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .weekwalk
        .mark_pop_shallow_settle(ctx.state.db)
        .await?;
    ctx.send_reply(CmdId::MarkPopShallowSettleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_show_finished(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = MarkShowFinishedRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .weekwalk
        .mark_show_finished(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::MarkShowFinishedCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_pop_deep_settle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    MarkPopDeepSettleRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .weekwalk
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

pub async fn on_weekwalk_ver2_mark_show_finished(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = WeekwalkVer2MarkShowFinishedRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .mark_weekwalk_v2_finished_shown(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::WeekwalkVer2MarkShowFinishedCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_weekwalk_ver2_change_hero_group_select(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = WeekwalkVer2ChangeHeroGroupSelectRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .change_weekwalk_v2_group(
            ctx.state.db,
            request.layer_id.ok_or(AppError::InvalidRequest)?,
            request.battle_id.ok_or(AppError::InvalidRequest)?,
            request.select.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(
        CmdId::WeekwalkVer2ChangeHeroGroupSelectCmd,
        reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_weekwalk_ver2_choose_skill(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = WeekwalkVer2ChooseSkillRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .exploration
        .choose_weekwalk_v2_skill(
            ctx.state.db,
            request.no.ok_or(AppError::InvalidRequest)?,
            request.skill_ids,
        )
        .await?;
    ctx.send_reply(CmdId::WeekwalkVer2ChooseSkillCmd, reply, 0, req.up_tag)
        .await
}
