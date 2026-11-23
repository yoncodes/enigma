use crate::util::push;
use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
};
use prost::Message;
use sonettobuf::{Buy107GoodsRequest, CmdId, Get107GoodsInfoRequest};

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

pub async fn on_buy_goods_107(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Buy107GoodsRequest::decode(&req.data[..])?;
    let purchase = ctx
        .player()?
        .activity
        .buy_act107_goods(ctx.state.db, msg.activity_id, msg.id, msg.num)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        purchase.rewards,
        purchase.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    ctx.send_reply(CmdId::Buy107GoodsCmd, purchase.reply, 0, req.up_tag)
        .await
}
