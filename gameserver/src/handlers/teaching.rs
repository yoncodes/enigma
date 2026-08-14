use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use logic::teaching;
use prost::Message;
use sonettobuf::{CmdId, TeachingGetInfoReply, TeachingGetInfoRequest};

pub async fn on_get_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    TeachingGetInfoRequest::decode(&req.data[..])?;
    let info = teaching::snapshot(ctx.state.db, player_id).await?;

    ctx.send_reply(
        CmdId::TeachingGetInfoCmd,
        TeachingGetInfoReply {
            teaching_info: Some(info),
        },
        0,
        req.up_tag,
    )
    .await
}
