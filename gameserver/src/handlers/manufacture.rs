use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::push,
};
use prost::Message;
use sonettobuf::{
    BuyManufactureBuildingRequest, CmdId, GetFrozenItemInfoRequest, GetManufactureInfoRequest,
    ManuBuildingUpgradeRequest, ManufactureAccelerateRequest, ReapFinishSlotRequest,
    SelectSlotProductionPlanRequest,
};
pub async fn on_get_manufacture_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    GetManufactureInfoRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .room
        .manufacture_info(ctx.state.db, ctx.state.tables)
        .await?;
    ctx.send_reply(CmdId::GetManufactureInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_frozen_item_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    GetFrozenItemInfoRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.room.frozen_item_info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetFrozenItemInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_buy_manufacture_building(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = BuyManufactureBuildingRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .room
        .buy_manufacture_building(
            ctx.state.db,
            msg.building_id.unwrap_or_default(),
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(
        CmdId::BuyManufactureBuildingCmd,
        update.reply,
        0,
        req.up_tag,
    )
    .await?;
    push::send_cost_pushes(ctx, player_id, update.item_ids, update.currency_ids).await
}

pub async fn on_manu_building_upgrade(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = ManuBuildingUpgradeRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .room
        .upgrade_manufacture_building(ctx.state.db, msg.uid.unwrap_or_default(), ctx.state.tables)
        .await?;
    ctx.send_reply(CmdId::ManuBuildingUpgradeCmd, update.reply, 0, req.up_tag)
        .await?;
    push::send_cost_pushes(ctx, player_id, update.item_ids, update.currency_ids).await
}

pub async fn on_select_slot_production_plan(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = SelectSlotProductionPlanRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .room
        .select_production_plan(
            ctx.state.db,
            msg.uid.unwrap_or_default(),
            &msg.operation_infos,
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::SelectSlotProductionPlanCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_manufacture_accelerate(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = ManufactureAccelerateRequest::decode(&req.data[..])?;
    let update = ctx
        .player()?
        .room
        .accelerate_manufacture(
            ctx.state.db,
            msg.uid.unwrap_or_default(),
            msg.slot_id.unwrap_or_default(),
            msg.use_item_data,
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::ManufactureAccelerateCmd, update.reply, 0, req.up_tag)
        .await?;
    push::send_cost_pushes(ctx, player_id, update.item_ids, update.currency_ids).await
}

pub async fn on_reap_finish_slot(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = ReapFinishSlotRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .room
        .reap_finished_slots(
            ctx.state.db,
            msg.building_uid.unwrap_or_default(),
            ctx.state.tables,
        )
        .await?;
    ctx.send_reply(CmdId::ReapFinishSlotCmd, reply, 0, req.up_tag)
        .await
}
