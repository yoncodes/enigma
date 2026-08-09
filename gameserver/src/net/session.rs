use crate::net::context::ConnectionContext;
use crate::net::{app::AppState, outbound::CommandPacket, router};
use crate::util::common::send_raw_server_message;
use byteorder::{BE, ByteOrder};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, tcp::OwnedWriteHalf},
    sync::mpsc,
};

#[allow(dead_code)]
pub async fn handle_client(socket: TcpStream, state: &'static AppState) -> anyhow::Result<()> {
    let (mut reader, writer) = socket.into_split();
    let (outbound_tx, outbound_rx) = mpsc::channel(64);
    let write_task = tokio::spawn(write_loop(writer, outbound_rx));
    let mut ctx = ConnectionContext::new(outbound_tx, state);

    let result = loop {
        let packet = {
            let mut header = [0u8; 4];
            if let Err(e) = reader.read_exact(&mut header).await {
                tracing::debug!("Client disconnected: {e}");
                break Ok(());
            }

            let packet_len = BE::read_i32(&header) as usize;
            let mut buffer = vec![0u8; packet_len];
            if let Err(e) = reader.read_exact(&mut buffer).await {
                tracing::warn!("Failed to read packet body ({} bytes): {e}", packet_len);
                break Ok(());
            }

            let mut packet = Vec::with_capacity(4 + packet_len);
            packet.extend_from_slice(&header);
            packet.extend_from_slice(&buffer);
            packet
        };

        if let Err(e) = router::dispatch_command(&mut ctx, packet).await {
            tracing::error!("Dispatch error: {e}");
            break Err(e.into());
        }
        if ctx.should_disconnect() {
            break Ok(());
        }
    };

    let saved_player_id = if let Ok(player) = ctx.player() {
        let player_id = player.id;
        let _session = ctx.state.lock_session(player_id).await;
        if ctx.state.is_current_session(player_id, &ctx.outbound) {
            if let Err(e) =
                database::db::game::power_maker::record_logout(ctx.state.db, player_id).await
            {
                tracing::error!(
                    "Failed to record power maker logout for {}: {}",
                    player_id,
                    e
                );
            }
            if let Err(e) = ctx.save_player().await {
                tracing::error!("Failed to save player state for {}: {}", player_id, e);
            }
            ctx.state
                .unregister_session_if_current(player_id, &ctx.outbound);
            Some(player_id)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(player_id) = saved_player_id {
        tracing::warn!("Player {} disconnected and saved progress", player_id);
    }

    drop(ctx);
    write_task.await??;
    result
}

async fn write_loop(
    mut writer: OwnedWriteHalf,
    mut rx: mpsc::Receiver<CommandPacket>,
) -> anyhow::Result<()> {
    while let Some(packet) = rx.recv().await {
        match packet {
            CommandPacket::Push {
                cmd_id,
                body,
                down_tag,
            } => {
                send_raw_server_message(&mut writer, cmd_id, body, 0, 255, down_tag).await?;
            }
            CommandPacket::Reply {
                cmd_id,
                body,
                result_code,
                up_tag,
                down_tag,
            } => {
                send_raw_server_message(&mut writer, cmd_id, body, result_code, up_tag, down_tag)
                    .await?;
            }
        }
    }

    writer.shutdown().await?;
    Ok(())
}
