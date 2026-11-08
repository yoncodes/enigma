use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    CmdId, GetPartyOutSideInfoRequest, PartyServerListReply, PartyServerListRequest,
    TriggerPartyResultReply, TriggerPartyResultRequest,
};

pub async fn on_get_outside_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetPartyOutSideInfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player()?
        .activity
        .party_outside_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetPartyOutSideInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_party_server_list(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    PartyServerListRequest::decode(&req.data[..])?;

    ctx.send_reply(
        CmdId::PartyServerListCmd,
        PartyServerListReply {
            party_servers: vec![],
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_trigger_result(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    TriggerPartyResultRequest::decode(&req.data[..])?;

    ctx.send_reply(
        CmdId::TriggerPartyResultCmd,
        TriggerPartyResultReply {},
        0,
        req.up_tag,
    )
    .await
}
