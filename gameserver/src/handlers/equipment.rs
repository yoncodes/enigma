use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::{push, task_events},
};
use logic::task::TaskEvent;
use prost::Message;
use sonettobuf::{
    CmdId, EquipBreakRequest, EquipDecomposeRequest, EquipDeletePush, EquipLockRequest,
    EquipRefineRequest, EquipStrengthenRequest,
};

pub async fn on_get_equip_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let inventory = ctx.player()?.inventory;
    let reply = inventory.equip_info(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetEquipInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_equip_lock(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let inventory = player.inventory;
    let msg = EquipLockRequest::decode(&req.data[..])?;
    let target_uid = msg.target_uid.ok_or(AppError::InvalidRequest)?;
    let lock = msg.lock.ok_or(AppError::InvalidRequest)?;
    let reply = inventory.equip_lock(ctx.state.db, target_uid, lock).await?;

    push::send_equip_update_push(ctx, player_id, vec![target_uid]).await?;
    ctx.send_reply(CmdId::EquipLockCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_equip_strengthen(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let inventory = player.inventory;
    let msg = EquipStrengthenRequest::decode(&req.data[..])?;
    let target_uid = msg.target_uid.ok_or(AppError::InvalidRequest)?;
    let strengthened = inventory
        .strengthen_equip(ctx.state.db, target_uid, msg.eat_equips)
        .await?;

    push::send_currency_change_push(ctx, player_id, strengthened.currency_changes).await?;
    push::send_equip_update_push(ctx, player_id, strengthened.changed_uids).await?;
    if !strengthened.deleted_uids.is_empty() {
        ctx.notify(
            CmdId::EquipDeletePushCmd,
            EquipDeletePush {
                uids: strengthened.deleted_uids,
            },
        )
        .await?;
    }
    task_events::notify(
        ctx,
        player_id,
        TaskEvent::DoneCount {
            name: "EquipStrengthen",
            count: 1,
        },
    )
    .await?;
    ctx.send_reply(CmdId::EquipStrengthenCmd, strengthened.reply, 0, req.up_tag)
        .await
}

pub async fn on_equip_break(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let inventory = player.inventory;
    let msg = EquipBreakRequest::decode(&req.data[..])?;
    let target_uid = msg.target_uid.ok_or(AppError::InvalidRequest)?;
    let (reply, changed_currencies, changed_items, changed_uids) =
        inventory.break_equip(ctx.state.db, target_uid).await?;

    push::send_currency_change_push(ctx, player_id, changed_currencies).await?;
    push::send_item_change_push(ctx, player_id, changed_items, Vec::new(), Vec::new()).await?;
    push::send_equip_update_push(ctx, player_id, changed_uids).await?;
    ctx.send_reply(CmdId::EquipBreakCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_equip_refine(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let inventory = player.inventory;
    let msg = EquipRefineRequest::decode(&req.data[..])?;
    let target_uid = msg.target_uid.ok_or(AppError::InvalidRequest)?;
    let (reply, changed_uids, delete_uids) = inventory
        .refine_equip(ctx.state.db, target_uid, msg.eat_uids)
        .await?;

    if !delete_uids.is_empty() {
        ctx.notify(
            CmdId::EquipDeletePushCmd,
            EquipDeletePush { uids: delete_uids },
        )
        .await?;
    }
    push::send_equip_update_push(ctx, player_id, changed_uids).await?;
    ctx.send_reply(CmdId::EquipRefineCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_equip_decompose(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let inventory = player.inventory;
    let msg = EquipDecomposeRequest::decode(&req.data[..])?;
    let (reply, changed_uids) = inventory
        .decompose_equips(ctx.state.db, msg.equip_uids.clone())
        .await?;

    ctx.notify(
        CmdId::EquipDeletePushCmd,
        EquipDeletePush {
            uids: msg.equip_uids,
        },
    )
    .await?;
    push::send_equip_update_push(ctx, player_id, changed_uids).await?;
    ctx.send_reply(CmdId::EquipDecomposeCmd, reply, 0, req.up_tag)
        .await
}

#[cfg(test)]
mod test;
