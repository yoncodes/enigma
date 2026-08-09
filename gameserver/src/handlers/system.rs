use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    session,
};
use prost::Message;
use sonettobuf::{CmdId, CritterInfoPush, RenameRequest, UpdateTaskPush};

pub async fn on_login(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let login = session::parse_login_request(&req.data)?;
    let db = ctx.state.db;

    let session = match session::validate_login(db, login).await {
        Ok(session) => session,
        Err(err) => {
            let payload = session::login_error_payload(&err.to_string());
            ctx.send_raw_reply_fixed(CmdId::LoginCmd, payload, 1, req.up_tag)
                .await?;
            return Ok(());
        }
    };

    let registration = ctx.state.lock_session(session.user_id).await;
    let updated_tasks = session::start_session(ctx, session).await?;
    let payload = session::login_reply_payload(session.user_id);
    ctx.send_raw_reply_fixed(CmdId::LoginCmd, payload, 0, req.up_tag)
        .await?;

    ctx.register();
    drop(registration);
    let critter_infos = ctx
        .player()?
        .critter
        .info(ctx.state.db)
        .await?
        .critter_infos;
    ctx.notify(CmdId::CritterInfoPushCmd, CritterInfoPush { critter_infos })
        .await?;
    if !updated_tasks.is_empty() {
        ctx.notify(
            CmdId::UpdateTaskPushCmd,
            UpdateTaskPush {
                task_info: updated_tasks.into_iter().map(Into::into).collect(),
                activity_info: Vec::new(),
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn on_reconnect(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    ctx.send_empty_reply(CmdId::ReconnectCmd, vec![0x01], 0, req.up_tag)
        .await
}

pub async fn on_rename(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let profile = ctx.player()?.profile;
    let msg = RenameRequest::decode(&req.data[..])?;
    let name = msg.name.unwrap_or_default();
    let guide_id = msg.guide_id.unwrap_or(1);
    let step_id = msg.step_id.unwrap_or(-1);
    let (reply, push) = profile
        .rename(ctx.state.db, name, guide_id, step_id)
        .await?;

    ctx.notify(CmdId::PlayerInfoPushCmd, push).await?;
    ctx.send_reply(CmdId::RenameCmd, reply, 0, req.up_tag).await
}
