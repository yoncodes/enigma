use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    session,
};
use prost::Message;
use sonettobuf::{
    Act116InfoUpdatePush, CmdId, CritterInfoPush, DeleteTaskPush, NewMailPush, RenameRequest,
    UpdateTaskPush,
};

pub async fn on_login(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    if ctx.logged_in {
        return Err(AppError::InvalidRequest);
    }
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
    let (updated_tasks, reset_task_ids, new_mails) = session::start_session(ctx, session).await?;
    let payload = session::login_reply_payload(session.user_id);
    ctx.send_raw_reply_fixed(CmdId::LoginCmd, payload, 0, req.up_tag)
        .await?;

    ctx.register_with_session_lock(registration);
    let critter_infos = ctx
        .player()?
        .critter
        .info(ctx.state.db)
        .await?
        .critter_infos;
    ctx.notify(CmdId::CritterInfoPushCmd, CritterInfoPush { critter_infos })
        .await?;
    let act116 = ctx
        .player()?
        .activity
        .act116_info(ctx.state.db, None, ctx.state.tables)
        .await?;
    ctx.notify(
        CmdId::Act116InfoUpdatePushCmd,
        Act116InfoUpdatePush {
            activity_id: act116.activity_id,
            infos: act116.infos,
            trap_ids: act116.trap_ids,
            put_trap: act116.put_trap,
            sp_status: act116.sp_status,
        },
    )
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
    if !reset_task_ids.is_empty() {
        ctx.notify(
            CmdId::DeleteTaskPushCmd,
            DeleteTaskPush {
                task_ids: reset_task_ids,
            },
        )
        .await?;
    }
    for mail in new_mails {
        ctx.notify(CmdId::NewMailPushCmd, NewMailPush { mail: Some(mail) })
            .await?;
    }
    Ok(())
}

pub async fn on_reconnect(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    reconnect_at(ctx, req, common::time::ServerTime::now_ms()).await
}

async fn reconnect_at(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
    now_ms: i64,
) -> Result<(), AppError> {
    session::reconcile_periodic_resets(ctx, now_ms).await?;
    ctx.player()?
        .activity
        .sync_act101_login_progress(ctx.state.db, now_ms)
        .await?;
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

#[cfg(test)]
mod tests;
