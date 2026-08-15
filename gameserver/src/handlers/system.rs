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
    reconnect_at(ctx, req, common::time::ServerTime::now_ms()).await
}

async fn reconnect_at(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
    now_ms: i64,
) -> Result<(), AppError> {
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
mod tests {
    use super::*;
    use crate::{
        net::{app::AppState, outbound::CommandPacket},
        player::{Player, PlayerState},
    };
    use database::db::game::activity101;
    use sqlx::SqlitePool;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn reconnect_advances_activity101_once_per_server_day() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let player_id = 540;
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'act101-reconnect', 0, 0)",
        )
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();

        let state = Box::leak(Box::new(AppState::new(pool, config::configs::get())));
        let (outbound, mut packets) = mpsc::channel(3);
        let mut ctx = ConnectionContext::new(outbound, state);
        ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
        let event_start = 1_786_615_200_000;

        for (sequence, now_ms) in [event_start, event_start + 1_000, event_start + 86_400_000]
            .into_iter()
            .enumerate()
        {
            reconnect_at(
                &mut ctx,
                ClientPacket {
                    sequence: sequence as i32,
                    cmd_id: CmdId::ReconnectCmd as i16,
                    up_tag: sequence as u8,
                    data: Vec::new(),
                },
                now_ms,
            )
            .await
            .unwrap();
            assert!(matches!(
                packets.try_recv().unwrap(),
                CommandPacket::Reply {
                    cmd_id: CmdId::ReconnectCmd,
                    result_code: 0,
                    ..
                }
            ));
        }

        assert_eq!(
            activity101::get_activity101_info(ctx.state.db, player_id, 13714)
                .await
                .unwrap()
                .1,
            2
        );
    }
}
