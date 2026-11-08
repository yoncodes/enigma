use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::{material_get_approach::MaterialGetApproach, red_dot_id::RedDotId},
    util::push,
};
use prost::Message;
use sonettobuf::{CmdId, GetMilestoneBonusRequest, GetMilestoneInfoRequest};

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

pub async fn on_get_bonus(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let request = GetMilestoneBonusRequest::decode(&req.data[..])?;
    let player_id = ctx.player()?.id;
    let claim = ctx
        .player()?
        .milestone
        .claim(
            ctx.state.db,
            request.milestone_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    push::send_item_first_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::MilestoneBonus),
    )
    .await?;
    push::clear_red_dots(
        ctx,
        [
            RedDotId::V3a9BossRushAct.id(),
            RedDotId::V3a9RacingCarReward.id(),
            RedDotId::SP02AtomicReward.id(),
        ],
    )
    .await?;
    ctx.send_reply(CmdId::GetMilestoneBonusCmd, claim.reply, 0, req.up_tag)
        .await
}
