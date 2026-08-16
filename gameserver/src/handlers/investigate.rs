use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{CmdId, GetInvestigateRequest, PutClueRequest};

pub async fn on_get_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    GetInvestigateRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .investigate
        .info(ctx.state.db, ctx.state.tables)
        .await?;

    ctx.send_reply(CmdId::GetInvestigateCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_put_clue(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let msg = PutClueRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .investigate
        .put_clue(
            ctx.state.db,
            ctx.state.tables,
            msg.id.ok_or(AppError::InvalidRequest)?,
            msg.clue_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    ctx.send_reply(CmdId::PutClueCmd, reply, 0, req.up_tag)
        .await
}
