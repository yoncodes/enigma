use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::push,
};
use prost::Message;
use sonettobuf::{
    AcceptAct186SpBonusRequest, Act186LikePush, CmdId, FinishAllAct186TaskRequest,
    GetAct186InfoRequest, GetAct186SpBonusInfoRequest,
};

pub async fn on_get_act186_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct186InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act186_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct186InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_act186_sp_bonus_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct186SpBonusInfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .get_act186_sp_bonus_info(db, msg.activity_id, msg.act186_activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct186SpBonusInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_accept_act186_sp_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = AcceptAct186SpBonusRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .accept_act186_sp_bonus(db, msg.activity_id, msg.act186_activity_id)
        .await?;

    ctx.send_reply(CmdId::AcceptAct186SpBonusCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_all_act186_tasks(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = FinishAllAct186TaskRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let claim = ctx
        .player_mut()?
        .activity
        .finish_all_act186_tasks(db, msg.activity_id)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::Activity),
    )
    .await?;
    if !claim.like_infos.is_empty() {
        ctx.notify(
            CmdId::Act186LikePushCmd,
            Act186LikePush {
                activity_id: claim.reply.activity_id,
                like_infos: claim.like_infos,
            },
        )
        .await?;
    }

    ctx.send_reply(CmdId::FinishAllAct186TaskCmd, claim.reply, 0, req.up_tag)
        .await
}
