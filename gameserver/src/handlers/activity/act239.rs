use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{Act239BonusRequest, CmdId, GetAct239InfoRequest};

pub async fn on_get_act239_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct239InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act239_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct239InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_act239_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act239BonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .act239_bonus(db, msg.activity_id, msg.id)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Act239Bonus),
    )
    .await?;
    let (red_dot_info_ids, red_dot_value) = if claim.red_dot_info_ids.is_empty() {
        (vec![0], 0)
    } else {
        (claim.red_dot_info_ids, 1)
    };
    push::send_red_dot_value_push(
        ctx,
        claim.red_dot_id,
        red_dot_info_ids,
        true,
        red_dot_value,
        0,
    )
    .await?;

    ctx.send_reply(CmdId::Act239BonusCmd, claim.reply, 0, req.up_tag)
        .await
}
