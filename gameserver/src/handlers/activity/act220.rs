use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::task_events,
};
use prost::Message;
use sonettobuf::{Act220FinishEpisodeRequest, CmdId, GetAct220InfoRequest};

pub async fn on_get_act220_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct220InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player()?
        .activity
        .act220_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct220InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_act220_episode(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Act220FinishEpisodeRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let finish = ctx
        .player()?
        .activity
        .finish_act220_episode(db, msg.activity_id, msg.episode_id, msg.progress)
        .await?;

    ctx.notify(CmdId::Act220EpisodePushCmd, finish.episode_push)
        .await?;
    task_events::notify_tasks(ctx, finish.tasks).await?;
    ctx.send_reply(CmdId::Act220FinishEpisodeCmd, finish.reply, 0, req.up_tag)
        .await
}
