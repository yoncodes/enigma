use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{CmdId, Get107GoodsInfoRequest};

pub async fn on_get_goods_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get107GoodsInfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player()?
        .activity
        .act107_goods_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::Get107GoodsInfoCmd, reply, 0, req.up_tag)
        .await
}
