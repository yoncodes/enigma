use super::*;

pub async fn on_production_line_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let rooms = ctx.player()?.room;
    let msg = ProductionLineInfoRequest::decode(&req.data[..])?;
    let reply = rooms.production_line_info(ctx.state.db, &msg.ids).await?;
    ctx.send_reply(CmdId::ProductionLineInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_start_production_line(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let rooms = ctx.player()?.room;
    let msg = StartProductionLineRequest::decode(&req.data[..])?;
    let line_id = msg.id.ok_or(AppError::InvalidRequest)?;
    let formula_id = msg
        .formula_produce
        .first()
        .and_then(|formula| formula.formula_id);
    let count = msg
        .formula_produce
        .first()
        .and_then(|formula| formula.count)
        .unwrap_or(1);
    let outcome = rooms
        .start_production_line(ctx.state.db, ctx.state.tables, line_id, formula_id, count)
        .await?;
    push::send_cost_pushes(
        ctx,
        player_id,
        outcome.consumed_item_ids,
        outcome.consumed_currency_ids,
    )
    .await?;
    task_events::notify(
        ctx,
        player_id,
        TaskEvent::ProductionLine {
            action: ProductionLineAction::Create,
            count,
        },
    )
    .await?;
    ctx.send_reply(CmdId::StartProductionLineCmd, outcome.reply, 0, req.up_tag)
        .await
}

pub async fn on_gain_production_line(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let rooms = ctx.player()?.room;
    let msg = GainProductionLineRequest::decode(&req.data[..])?;
    let outcome = rooms
        .gain_production_line(ctx.state.db, ctx.state.tables, &msg.ids)
        .await?;
    let gathered_count = outcome.reply.production_lines.len() as i32;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        outcome.rewards,
        outcome.material_changes,
        Some(MaterialGetApproach::RoomProductLine),
    )
    .await?;
    if gathered_count > 0 {
        task_events::notify(
            ctx,
            player_id,
            TaskEvent::ProductionLine {
                action: ProductionLineAction::Gather,
                count: gathered_count,
            },
        )
        .await?;
    }
    ctx.send_reply(CmdId::GainProductionLineCmd, outcome.reply, 0, req.up_tag)
        .await
}

pub async fn on_production_line_lv_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let rooms = ctx.player()?.room;
    let msg = ProductionLineLvUpRequest::decode(&req.data[..])?;
    let reply = rooms
        .production_line_lv_up(
            ctx.state.db,
            ctx.state.tables,
            msg.id.ok_or(AppError::InvalidRequest)?,
            msg.new_level.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    push::send_cost_pushes(
        ctx,
        player_id,
        reply.consumed_item_ids,
        reply.consumed_currency_ids,
    )
    .await?;
    ctx.send_reply(CmdId::ProductionLineLvUpCmd, reply.reply, 0, req.up_tag)
        .await
}

pub async fn on_room_level_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let rooms = ctx.player()?.room;
    let reply = rooms.room_level_up(ctx.state.db, ctx.state.tables).await?;
    push::send_cost_pushes(
        ctx,
        player_id,
        reply.consumed_item_ids,
        reply.consumed_currency_ids,
    )
    .await?;
    let open_infos = ctx
        .player()?
        .profile
        .reconcile_open_infos(ctx.state.db)
        .await?;
    if !open_infos.is_empty() {
        ctx.notify(CmdId::UpdateOpenPushCmd, UpdateOpenPush { open_infos })
            .await?;
    }
    let tasks = ctx
        .player()?
        .room
        .sync_room_tasks(ctx.state.db, ctx.state.tables)
        .await?;
    task_events::notify_tasks(ctx, tasks).await?;
    ctx.send_reply(CmdId::RoomLevelUpCmd, reply.reply, 0, req.up_tag)
        .await
}

pub async fn on_production_line_accelerate(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let rooms = ctx.player()?.room;
    let msg = ProductionLineAccelerateRequest::decode(&req.data[..])?;
    let reply = rooms
        .production_line_accelerate(ctx.state.db, msg.id.ok_or(AppError::InvalidRequest)?)
        .await?;
    ctx.send_reply(CmdId::ProductionLineAccelerateCmd, reply, 0, req.up_tag)
        .await
}
