use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::{push, task_events},
};
use prost::Message;
use sonettobuf::{
    CmdId, FinishNecrologistStoryModeReply, FinishNecrologistStoryModeRequest, GetHeroStoryRequest,
    GetNecrologistStoryRequest, GetStoryFinishRequest, HeroStoryPlotFinishRequest, StoryFinishPush,
    UnlocHeroStoryRequest, UpdateNecrologistStoryRequest, UpdateStoryRequest,
};

pub async fn on_get_story(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let reply = ctx.player()?.story.get(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetStoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_story_finish(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetStoryFinishRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .story
        .finish_state(ctx.state.db, msg.story_id.ok_or(AppError::InvalidRequest)?)
        .await?;
    ctx.send_reply(CmdId::GetStoryFinishCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_story(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UpdateStoryRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .story
        .update(
            ctx.state.db,
            msg.story_id.unwrap_or_default(),
            msg.step_id.unwrap_or_default(),
            msg.favor.unwrap_or_default(),
        )
        .await?;
    if let Some(story_id) = update.finished_story_id {
        ctx.notify(
            CmdId::StoryFinishPushCmd,
            StoryFinishPush {
                story_id: Some(story_id),
            },
        )
        .await?;
        push::send_dungeon_map_progression(ctx, player_id).await?;
    }
    ctx.send_reply(CmdId::UpdateStoryCmd, update.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_hero_story(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    GetHeroStoryRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.story.hero_story(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetHeroStoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_unlock_hero_story(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UnlocHeroStoryRequest::decode(&req.data[..])?;
    let (reply, consumed) = ctx
        .player()?
        .story
        .unlock_hero_story(ctx.state.db, msg.story_id.ok_or(AppError::InvalidRequest)?)
        .await?;
    push::send_cost_pushes(ctx, player_id, consumed.item_ids, consumed.currency_ids).await?;
    ctx.send_reply(CmdId::UnlocHeroStoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_necrologist_story(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetNecrologistStoryRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .story
        .necrologist_story(
            ctx.state.db,
            msg.story_id.unwrap_or_default(),
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::GetNecrologistStoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_hero_story_plot(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = HeroStoryPlotFinishRequest::decode(&req.data[..])?;
    let (reply, event) = ctx
        .player()?
        .story
        .finish_hero_story_plot(
            ctx.state.db,
            msg.plot.ok_or(AppError::InvalidRequest)?,
            ctx.state.tables,
        )
        .await?;
    task_events::notify(ctx, player_id, event).await?;
    ctx.send_reply(CmdId::HeroStoryPlotFinishCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_necrologist_story(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = UpdateNecrologistStoryRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .story
        .update_necrologist_story(
            ctx.state.db,
            msg.story_id.unwrap_or_default(),
            msg.info.unwrap_or_else(|| "{}".to_string()),
            msg.plot_infos,
        )
        .await?;
    ctx.send_reply(CmdId::UpdateNecrologistStoryCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_necrologist_story_mode(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = FinishNecrologistStoryModeRequest::decode(&req.data[..])?;
    let reply = FinishNecrologistStoryModeReply {
        story_id: msg.story_id,
        mode_id: msg.mode_id,
    };
    ctx.send_reply(CmdId::FinishNecrologistStoryModeCmd, reply, 0, req.up_tag)
        .await
}
