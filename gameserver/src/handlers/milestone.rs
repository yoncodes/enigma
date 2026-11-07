use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{CmdId, GetMilestoneInfoRequest};

pub async fn on_get_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = GetMilestoneInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .milestone
        .info(ctx.state.db, request.milestone_ids)
        .await?;
    ctx.send_reply(CmdId::GetMilestoneInfoCmd, reply, 0, req.up_tag)
        .await
}
