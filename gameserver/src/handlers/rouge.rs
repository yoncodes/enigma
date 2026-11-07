use crate::{
    error::AppError,
    logic::rouge,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    CmdId, EnterRouge2SelectDifficultyRequest, GetRouge2InfoRequest, GetRouge2OutsideInfoRequest,
    GetRougeOutsideInfoRequest, Rouge2AlchemyRequest, Rouge2AttrUpdatePush,
    Rouge2GetUnlockCollectionsRequest, Rouge2UpdateGeniusPointPush,
};

pub async fn on_get_rouge_outside_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetRougeOutsideInfoRequest::decode(&req.data[..])?;
    let reply = rouge::rouge_outside_info(msg.season.unwrap_or_default());
    ctx.send_reply(CmdId::GetRougeOutsideInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_rouge2_outside_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let _ = GetRouge2OutsideInfoRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.rouge.outside_info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetRouge2OutsideInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_rouge2_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let _ = GetRouge2InfoRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.rouge.info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetRouge2InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_enter_rouge2_select_difficulty(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = EnterRouge2SelectDifficultyRequest::decode(&req.data[..])?;
    let result = ctx
        .player()?
        .rouge
        .select_difficulty(
            ctx.state.db,
            msg.difficulty.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    ctx.notify(
        CmdId::Rouge2AttrUpdatePushCmd,
        Rouge2AttrUpdatePush {
            updates: result.updates,
        },
    )
    .await?;
    ctx.send_reply(
        CmdId::EnterRouge2SelectDifficultyCmd,
        result.reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_rouge2_get_unlock_collections(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let _ = Rouge2GetUnlockCollectionsRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.rouge.unlock_collections(ctx.state.db).await?;
    ctx.send_reply(CmdId::Rouge2GetUnlockCollectionsCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_rouge2_alchemy(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Rouge2AlchemyRequest::decode(&req.data[..])?;
    let formula = msg.formula.ok_or(AppError::InvalidRequest)?;
    let result = ctx
        .player()?
        .rouge
        .alchemy(ctx.state.db, formula, msg.sub_material)
        .await?;
    let achievement_push = ctx
        .player_mut()?
        .collection
        .record_updates(result.achievements);

    ctx.notify(
        CmdId::Rouge2UpdateGeniusPointPushCmd,
        Rouge2UpdateGeniusPointPush {
            genius_point: Some(result.genius_point),
        },
    )
    .await?;
    if let Some(push) = achievement_push {
        ctx.notify(CmdId::UpdateAchievementPushCmd, push).await?;
    }
    ctx.send_reply(CmdId::Rouge2AlchemyCmd, result.reply, 0, req.up_tag)
        .await
}
