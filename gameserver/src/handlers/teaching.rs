use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::red_dot_id::RedDotId,
    util::push,
};
use logic::teaching;
use prost::Message;
use sonettobuf::{
    CmdId, Teaching, TeachingGetBonusReply, TeachingGetBonusRequest, TeachingGetInfoReply,
    TeachingGetInfoRequest,
};

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

pub async fn on_get_bonus(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TeachingGetBonusRequest::decode(&req.data[..])?;
    let [teaching_id] = request.teaching_ids.as_slice() else {
        return Err(AppError::InvalidRequest);
    };
    let claim = teaching::claim_bonus(ctx.state.db, player_id, *teaching_id).await?;

    push::send_currency_change_push(ctx, player_id, claim.rewards.currency_ids).await?;
    push::send_material_change_push_raw(ctx, claim.material_changes, Some(170)).await?;
    push::clear_red_dots(ctx, [RedDotId::TeachingSystem.id()]).await?;
    ctx.send_reply(
        CmdId::TeachingGetBonusCmd,
        TeachingGetBonusReply {
            teachinges: vec![Teaching {
                teaching_id: Some(claim.teaching_id),
                status: Some(2),
            }],
        },
        0,
        req.up_tag,
    )
    .await
}
