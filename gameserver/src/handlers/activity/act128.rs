use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::{material_get_approach::MaterialGetApproach, red_dot_id::RedDotId},
    util::push,
};
use prost::Message;
use sonettobuf::{Act128GetMilestoneBonusRequest, CmdId};

pub async fn on_act128_get_milestone_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act128GetMilestoneBonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .get_act128_milestone_bonus(db, msg.activity_id)
        .await?;

    push::send_item_first_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Act128MilestoneBonus),
    )
    .await?;
    push::send_red_dot_value_push(ctx, RedDotId::BossRushRankBonus.id(), vec![0], true, 0, 0)
        .await?;

    ctx.send_reply(
        CmdId::Act128GetMilestoneBonusCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}
