use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{Act236GetAutoGainRewardRequest, CmdId};

pub async fn on_act236_get_auto_gain_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = Act236GetAutoGainRewardRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .act236_get_auto_gain_reward(db, msg.activity_id, msg.reward_ids)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Act236Reward),
    )
    .await?;
    push::send_red_dot_value_push(ctx, claim.red_dot_id, vec![0], true, claim.red_dot_value, 0)
        .await?;

    ctx.send_reply(
        CmdId::Act236GetAutoGainRewardCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}
