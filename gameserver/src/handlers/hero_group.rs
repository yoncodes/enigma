use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    ChangeHeroGroupSelectRequest, CheckHeroGroupNameRequest, CmdId, DeleteHeroGroupRequest,
    GetHeroGroupSnapshotListRequest, SetHeroGroupEquipRequest, SetHeroGroupSnapshotBatchRequest,
    SetHeroGroupSnapshotRequest, UpdateHeroGroupNameRequest, UpdateHeroGroupRequest,
    UpdateHeroGroupSortRequest,
};

pub async fn on_check_hero_group_name(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = CheckHeroGroupNameRequest::decode(&req.data[..])?;
    let reply = heroes.check_group_name(msg.name.as_deref().unwrap_or_default())?;

    ctx.send_reply(CmdId::CheckHeroGroupNameCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_hero_group_name(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = UpdateHeroGroupNameRequest::decode(&req.data[..])?;
    let reply = heroes
        .update_group_name(
            ctx.state.db,
            msg.id.ok_or(AppError::InvalidRequest)?,
            msg.current_select.ok_or(AppError::InvalidRequest)?,
            msg.name.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    ctx.send_reply(CmdId::UpdateHeroGroupNameCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_hero_group_sort(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = UpdateHeroGroupSortRequest::decode(&req.data[..])?;
    let reply = heroes
        .update_group_sort(
            ctx.state.db,
            msg.snapshot_id.ok_or(AppError::InvalidRequest)?,
            msg.sort_sub_ids,
        )
        .await?;
    ctx.send_reply(CmdId::UpdateHeroGroupSortCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_delete_hero_group(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = DeleteHeroGroupRequest::decode(&req.data[..])?;
    let reply = heroes
        .delete_group(
            ctx.state.db,
            msg.snapshot_id.ok_or(AppError::InvalidRequest)?,
            msg.snapshot_sub_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::DeleteHeroGroupCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_update_hero_group(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = UpdateHeroGroupRequest::decode(&req.data[..])?;
    let reply = heroes.update_group(ctx.state.db, msg.group_info).await?;
    ctx.send_reply(CmdId::HeroGroupUpdateHeroGroupCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_hero_group_common_list(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx.player()?.hero.group_common_list(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetHeroGroupCommonListCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_hero_group_list(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let reply = ctx.player()?.hero.group_list(ctx.state.db).await?;
    ctx.send_reply(CmdId::GetHeroGroupListCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_hero_group_snapshot_list(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = GetHeroGroupSnapshotListRequest::decode(&req.data[..])?;
    let reply = heroes
        .group_snapshot_list(ctx.state.db, msg.snapshot_id)
        .await?;
    ctx.send_reply(CmdId::GetHeroGroupSnapshotListCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_set_hero_group_equip(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = SetHeroGroupEquipRequest::decode(&req.data[..])?;
    let group_id = msg.group_id.ok_or(AppError::InvalidRequest)?;
    let equip = msg.equip.ok_or(AppError::InvalidRequest)?;
    let reply = heroes
        .set_group_equip(ctx.state.db, group_id, equip)
        .await?;

    ctx.send_reply(CmdId::SetHeroGroupEquipCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_set_hero_group_snapshot(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = SetHeroGroupSnapshotRequest::decode(&req.data[..])?;
    let snapshot_id = msg.snapshot_id.ok_or(AppError::InvalidRequest)?;
    let snapshot_sub_id = msg.snapshot_sub_id.unwrap_or_default();
    let fight_group = msg.fight_group.ok_or(AppError::InvalidRequest)?;
    let mut tx = ctx.state.db.begin().await?;
    let reply = heroes
        .set_group_snapshot_in_transaction(&mut tx, snapshot_id, snapshot_sub_id, fight_group)
        .await?;
    let stage_push = ctx
        .player()?
        .activity
        .touch_act229_stage_teams(&mut tx, snapshot_id, &[snapshot_sub_id])
        .await?;
    tx.commit().await?;

    ctx.send_reply(CmdId::SetHeroGroupSnapshotCmd, reply, 0, req.up_tag)
        .await?;
    if let Some(push) = stage_push {
        ctx.notify(CmdId::Act229StageTeamSubIdPushCmd, push).await?;
    }
    Ok(())
}

pub async fn on_set_hero_group_snapshot_batch(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = SetHeroGroupSnapshotBatchRequest::decode(&req.data[..])?;
    let snapshot_id = msg.snapshot_id.ok_or(AppError::InvalidRequest)?;
    let mut tx = ctx.state.db.begin().await?;
    let reply = heroes
        .set_group_snapshot_batch_in_transaction(&mut tx, snapshot_id, msg.fight_groups)
        .await?;
    let sub_ids = reply
        .group_infos
        .iter()
        .filter_map(|group| group.snapshot_sub_id)
        .collect::<Vec<_>>();
    let stage_push = ctx
        .player()?
        .activity
        .touch_act229_stage_teams(&mut tx, snapshot_id, &sub_ids)
        .await?;
    tx.commit().await?;

    ctx.send_reply(CmdId::SetHeroGroupSnapshotBatchCmd, reply, 0, req.up_tag)
        .await?;
    if let Some(push) = stage_push {
        ctx.notify(CmdId::Act229StageTeamSubIdPushCmd, push).await?;
    }
    Ok(())
}

pub async fn on_change_hero_group_select(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let msg = ChangeHeroGroupSelectRequest::decode(&req.data[..])?;
    let reply = heroes
        .change_group_selection(
            ctx.state.db,
            msg.id.ok_or(AppError::InvalidRequest)?,
            msg.current_select.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    ctx.send_reply(CmdId::ChangeHeroGroupSelectCmd, reply, 0, req.up_tag)
        .await
}
