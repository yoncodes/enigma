use crate::{
    dungeon,
    error::AppError,
    net::context::ConnectionContext,
    net::packet::ClientPacket,
    player::battle::ActiveBattle,
    tower, tower_compose,
    util::{push, task_events},
};
use config::configs;
use logic::{task::TaskEvent, types::material_get_approach::MaterialGetApproach};
use prost::Message;
use sonettobuf::{
    AutoRoundRequest, BeginRoundRequest, CmdId, CoverDungeonRecordReply, CoverDungeonRecordRequest,
    DungeonInfosPush, EndDungeonReply, EndDungeonRequest, EndFightReply, EndFightRequest,
    EntityInfoReply, EntityInfoRequest, GetFightCardDeckDetailInfoReply,
    GetFightCardDeckDetailInfoRequest, GetFightCardDeckInfoReply, GetFightCardDeckInfoRequest,
    GetFightOperReply, GetFightOperRequest, GetFightRecordGroupReply, GetFightRecordGroupRequest,
    GetMapElementRecordRequest, GetPointRewardRequest, GetPuzzleProgressRequest,
    InstructionDungeonFinalRewardRequest, InstructionDungeonInfoRequest,
    InstructionDungeonOpenRequest, InstructionDungeonRewardRequest, MapElementRequest,
    PuzzleFinishRequest, ReconnectFightRequest, RefreshAssistRequest, ResetRoundRequest,
    RewardPointUpdatePush, SavePuzzleProgressRequest, StartDungeonReply, StartDungeonRequest,
    UpdateOpenPush, UseClothSkillRequest,
};

pub async fn on_refresh_assist(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = RefreshAssistRequest::decode(&req.data[..])?;
    let reply = dungeon::refresh_assist(ctx.state.db, player_id, request).await?;
    ctx.send_reply(CmdId::RefreshAssistCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_entity_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = EntityInfoRequest::decode(&req.data[..])?;
    let entity_info = ctx
        .player()?
        .battle
        .entity_info(request.uid.ok_or(AppError::InvalidRequest)?)?;
    ctx.send_reply(
        CmdId::EntityInfoCmd,
        EntityInfoReply {
            entity_info: Some(entity_info),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_get_fight_card_deck_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = GetFightCardDeckInfoRequest::decode(&req.data[..])?;
    let deck_infos = ctx
        .player()?
        .battle
        .card_deck(request.r#type.unwrap_or_default())?;
    ctx.send_reply(
        CmdId::GetFightCardDeckInfoCmd,
        GetFightCardDeckInfoReply {
            deck_infos,
            device_infos: Vec::new(),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_get_fight_card_deck_detail_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = GetFightCardDeckDetailInfoRequest::decode(&req.data[..])?;
    let deck_infos = ctx
        .player()?
        .battle
        .card_deck(request.r#type.unwrap_or_default())?;
    ctx.send_reply(
        CmdId::GetFightCardDeckDetailInfoCmd,
        GetFightCardDeckDetailInfoReply {
            deck_infos,
            device_infos: Vec::new(),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_get_puzzle_progress(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = GetPuzzleProgressRequest::decode(&req.data[..])?;
    let reply = dungeon::get_puzzle_progress(
        ctx.state.db,
        player_id,
        msg.element_id.ok_or(AppError::InvalidRequest)?,
    )
    .await?;
    ctx.send_reply(CmdId::GetPuzzleProgressCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_save_puzzle_progress(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = SavePuzzleProgressRequest::decode(&req.data[..])?;
    let reply = dungeon::save_puzzle_progress(
        ctx.state.db,
        player_id,
        msg.element_id.ok_or(AppError::InvalidRequest)?,
        msg.progress.ok_or(AppError::InvalidRequest)?,
    )
    .await?;
    ctx.send_reply(CmdId::SavePuzzleProgressCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_puzzle_finish(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = PuzzleFinishRequest::decode(&req.data[..])?;
    let reply = dungeon::finish_puzzle(
        ctx.state.db,
        player_id,
        msg.element_id.ok_or(AppError::InvalidRequest)?,
    )
    .await?;
    ctx.send_reply(CmdId::PuzzleFinishCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_map_element(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = MapElementRequest::decode(&req.data[..])?;
    let completion = logic::dungeon::DungeonManager::new(player_id)
        .complete_map_element(
            ctx.state.db,
            request.element_id.ok_or(AppError::InvalidRequest)?,
            request.dialog_ids,
            request.record.unwrap_or_default(),
        )
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        completion.rewards,
        completion.material_changes,
        Some(MaterialGetApproach::Explore),
    )
    .await?;
    if let Some((chapter_id, value)) = completion.reward_point {
        ctx.notify(
            CmdId::RewardPointUpdatePushCmd,
            RewardPointUpdatePush {
                chapter_id: Some(chapter_id),
                value: Some(value),
            },
        )
        .await?;
    }
    ctx.send_reply(CmdId::MapElementCmd, completion.reply, 0, req.up_tag)
        .await?;
    push::send_dungeon_map_progression(ctx, player_id).await
}

pub async fn on_get_map_element_record(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = GetMapElementRecordRequest::decode(&req.data[..])?;
    let reply = logic::dungeon::DungeonManager::new(player_id)
        .map_element_records(ctx.state.db, request.element_ids)
        .await?;
    ctx.send_reply(CmdId::GetMapElementRecordCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_reset_round(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    ResetRoundRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.battle.reset_round()?;
    ctx.send_reply(CmdId::ResetRoundCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_use_cloth_skill(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = UseClothSkillRequest::decode(&req.data[..])?;
    let (reply, redeal) = ctx.player_mut()?.battle.use_cloth_skill(request)?;
    if let Some(redeal) = redeal {
        ctx.notify(CmdId::RedealCardInfoPushCmd, redeal).await?;
    }
    ctx.send_reply(CmdId::UseClothSkillCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_dungeon(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let (reply, dungeons) = dungeon::dungeon_info(ctx.state.db, player_id).await?;

    ctx.send_reply(CmdId::GetDungeonCmd, reply, 0, req.up_tag)
        .await?;

    for chunk in dungeons.chunks(100) {
        ctx.notify(
            CmdId::DungeonInfosPushCmd,
            DungeonInfosPush {
                dungeon_infos: chunk.iter().cloned().map(Into::into).collect(),
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn on_get_point_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = GetPointRewardRequest::decode(&req.data[..])?;
    let claim = logic::dungeon::DungeonManager::new(player_id)
        .claim_point_rewards(ctx.state.db, request.id)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(MaterialGetApproach::DungeonRewardPoint),
    )
    .await?;
    ctx.send_reply(CmdId::GetPointRewardCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_start_dungeon(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = StartDungeonRequest::decode(&req.data[..])?;
    if let Some(matches) = ctx.player()?.battle.saved_start_matches(&request) {
        if !matches {
            return Err(AppError::InvalidRequest);
        }
        if !request.is_restart.unwrap_or(false) {
            return send_active_battle_start(ctx, req.up_tag).await;
        }
    }

    match dungeon::restore_active_fight(ctx.state.db, player_id).await? {
        dungeon::ActiveFightRestore::Missing => {
            if ctx.player()?.battle.has_active() {
                return Err(AppError::InvalidRequest);
            }
        }
        dungeon::ActiveFightRestore::Active(active) => {
            if !dungeon::matches_saved_dungeon_start(&active, &request) {
                return Err(AppError::InvalidRequest);
            }
            ctx.player_mut()?.battle.restore_active(*active);
            return send_active_battle_start(ctx, req.up_tag).await;
        }
        dungeon::ActiveFightRestore::Refunded(refund) => {
            ctx.player_mut()?.battle.clear_active();
            send_refund(ctx, player_id, *refund).await?;
        }
    }

    let episode_id = request.episode_id.unwrap_or(0);
    let multiplier = request.multiplication.unwrap_or(1).max(1);

    let game_data = configs::get();
    let episode_cfg = game_data
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let chapter_id = request.chapter_id.unwrap_or(episode_cfg.chapter_id);
    if !dungeon::can_start_episode(ctx.state.db, player_id, chapter_id, episode_id).await? {
        return Err(AppError::InvalidRequest);
    }

    let battle_id = dungeon::episode_battle_id(ctx.state.db, player_id, episode_cfg).await?;
    if battle_id == 0 {
        let settlement = dungeon::settle_battleless(
            ctx.state.db,
            player_id,
            chapter_id,
            episode_id,
            dungeon::DungeonCompletion {
                star: 1,
                total_round: 0,
                multiplier,
                fight_group: None,
            },
            &Default::default(),
        )
        .await?;
        push::send_cost_pushes(
            ctx,
            player_id,
            settlement.cost.item_ids,
            settlement.cost.currency_ids,
        )
        .await?;
        ctx.player_mut()?.battle.clear_pending_record();
        send_completed_dungeon(ctx, player_id, chapter_id, episode_id, settlement.dungeon).await?;

        return ctx
            .send_reply(
                CmdId::StartDungeonCmd,
                StartDungeonReply {
                    fight: None,
                    round: None,
                },
                0,
                req.up_tag,
            )
            .await;
    }

    let mut active =
        ActiveBattle::prepare(ctx.state.db, player_id, episode_id, battle_id, request).await?;
    let cost = active
        .activate(
            ctx.state.db,
            player_id,
            &dungeon::episode_cost(episode_cfg, multiplier),
        )
        .await?;
    ctx.player_mut()?.battle.start_active(active);

    push::send_cost_pushes(ctx, player_id, cost.item_ids, cost.currency_ids).await?;
    send_active_battle_start(ctx, req.up_tag).await
}

pub async fn on_reconnect_fight(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    ReconnectFightRequest::decode(&req.data[..])?;
    if !ctx.player()?.battle.has_active() {
        match dungeon::restore_active_fight(ctx.state.db, player_id).await? {
            dungeon::ActiveFightRestore::Missing => {}
            dungeon::ActiveFightRestore::Active(active) => {
                ctx.player_mut()?.battle.restore_active(*active);
            }
            dungeon::ActiveFightRestore::Refunded(refund) => {
                send_refund(ctx, player_id, *refund).await?;
            }
        }
    }
    let reply = ctx.player()?.battle.reconnect_reply();
    ctx.send_reply(CmdId::ReconnectFightCmd, reply, 0, req.up_tag)
        .await
}

async fn send_active_battle_start(ctx: &mut ConnectionContext, up_tag: u8) -> Result<(), AppError> {
    let (reply, cards) = ctx.player()?.battle.start_payload()?;
    ctx.send_reply(CmdId::StartDungeonCmd, reply, 0, up_tag)
        .await?;
    ctx.notify(CmdId::CardInfoPushCmd, cards).await
}

pub async fn on_instruction_dungeon_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    InstructionDungeonInfoRequest::decode(&req.data[..])?;
    let reply = dungeon::instruction_dungeon_info(ctx.state.db, player_id).await?;

    ctx.send_reply(
        CmdId::DungeonInstructionDungeonInfoCmd,
        reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_instruction_dungeon_open(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = InstructionDungeonOpenRequest::decode(&req.data[..])?;
    let (reply, changed) =
        dungeon::instruction_dungeon_open(ctx.state.db, player_id, msg.open_id).await?;

    if changed {
        push::send_instruction_dungeon_info(ctx, player_id).await?;
    }
    ctx.send_reply(CmdId::InstructionDungeonOpenCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_instruction_dungeon_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = InstructionDungeonRewardRequest::decode(&req.data[..])?;
    let claim = dungeon::instruction_dungeon_reward(
        ctx.state.db,
        player_id,
        msg.topic_id.unwrap_or_default(),
    )
    .await?;

    push::send_applied_reward_pushes(ctx, player_id, claim.rewards, claim.material_changes, None)
        .await?;
    push::send_instruction_dungeon_info(ctx, player_id).await?;
    ctx.send_reply(
        CmdId::InstructionDungeonRewardCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_instruction_dungeon_final_reward(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    InstructionDungeonFinalRewardRequest::decode(&req.data[..])?;
    let claim = dungeon::instruction_dungeon_final_reward(ctx.state.db, player_id).await?;

    push::send_applied_reward_pushes(ctx, player_id, claim.rewards, claim.material_changes, None)
        .await?;
    push::send_instruction_dungeon_info(ctx, player_id).await?;
    ctx.send_reply(
        CmdId::InstructionDungeonFinalRewardCmd,
        claim.reply,
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_begin_round(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let request = BeginRoundRequest::decode(&req.data[..])?;
    let reply = ctx.player_mut()?.battle.begin_round(request)?;

    ctx.send_reply(CmdId::BeginRoundCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_auto_round(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let msg = AutoRoundRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.battle.plan_auto_round(&msg)?;

    ctx.send_reply(CmdId::AutoRoundCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_fight_end_fight(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = EndFightRequest::decode(&req.data[..])?;
    let active = ctx.player()?.battle.active_snapshot();
    let end = active.as_ref().map(|active| {
        if msg.is_abort.unwrap_or(false) {
            dungeon::abort_end_fight(active)
        } else {
            dungeon::completed_end_fight(active)
        }
    });

    let mut compose_push = None;
    let mut dungeon_settlement = None;
    let mut refund_settlement = None;
    let mut act229_finish = None;
    if let Some(active) = active.as_ref() {
        let is_abort = msg.is_abort.unwrap_or(false);
        let compose_handled = tower_compose::matches_battle(active);
        let won = active.is_victory();

        if !is_abort
            && won
            && (compose_handled || (active.tower_type.is_none() && active.act229_context.is_none()))
        {
            let star = active.star();
            let round = active.current_round();
            let record =
                dungeon::prepare_dungeon_record(ctx.state.db, player_id, active, round).await?;
            let mut settlement = dungeon::settle_active(
                ctx.state.db,
                player_id,
                active,
                dungeon::DungeonCompletion {
                    star,
                    total_round: round,
                    multiplier: active.multiplication.unwrap_or(1).max(1),
                    fight_group: active.fight_group.as_ref(),
                },
                &record,
            )
            .await?;
            compose_push = settlement.compose_push.take();
            ctx.player_mut()?.battle.complete_active(record.pending);
            dungeon_settlement = Some((settlement, active.chapter_id, active.episode_id));
        } else if is_abort || !won {
            let mut settlement = if is_abort {
                dungeon::settle_refund(ctx.state.db, player_id, active, false).await?
            } else {
                dungeon::settle_failed(ctx.state.db, player_id, active, true).await?
            };
            compose_push = settlement.compose_push.take();
            refund_settlement = Some(settlement);
        } else if let Some(fight_id) = active.fight_id {
            dungeon::finish_fight_instance(ctx.state.db, player_id, fight_id).await?;
        }

        if !is_abort
            && won
            && let Some(context) = active.act229_context
        {
            act229_finish = Some(
                ctx.player()?
                    .activity
                    .finish_act229_battle(
                        ctx.state.db,
                        context.activity_id,
                        context.stage_id,
                        active.current_round(),
                        active.star(),
                        &active.act229_heroes(),
                    )
                    .await?,
            );
        }
        ctx.player_mut()?.battle.clear_active();
    }
    if let Some(settle) = compose_push {
        ctx.notify(CmdId::TowerComposeFightSettlePushCmd, settle)
            .await?;
    }
    if let Some((settlement, chapter_id, episode_id)) = dungeon_settlement {
        send_completed_dungeon(ctx, player_id, chapter_id, episode_id, settlement).await?;
    }
    if let Some(settlement) = refund_settlement {
        send_refund(ctx, player_id, settlement).await?;
    }
    if let Some(finish) = act229_finish {
        ctx.notify(CmdId::Act229BattleFinishPushCmd, finish).await?;
    }
    if let Some(end) = end {
        push::send_end_fight_push(ctx, end).await?;
    }
    ctx.send_reply(CmdId::FightEndFightCmd, EndFightReply {}, 0, req.up_tag)
        .await
}

pub async fn on_get_fight_record_group(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = GetFightRecordGroupRequest::decode(&req.data[..])?;
    let fight_group = dungeon::load_dungeon_record(
        ctx.state.db,
        player_id,
        request.episode_id.unwrap_or_default(),
    )
    .await?;

    ctx.send_reply(
        CmdId::GetFightRecordGroupCmd,
        GetFightRecordGroupReply { fight_group },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_get_fight_oper(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    GetFightOperRequest::decode(&req.data[..])?;
    let episode_id = ctx.player()?.battle.replay_episode_id()?;
    let oper_records =
        dungeon::load_dungeon_record_operations(ctx.state.db, player_id, episode_id).await?;

    ctx.send_reply(
        CmdId::GetFightOperCmd,
        GetFightOperReply { oper_records },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_dungeon_end_dungeon(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = EndDungeonRequest::decode(&req.data[..])?;
    let active = ctx.player()?.battle.active_snapshot();

    if let Some(active) = active.as_ref() {
        if msg.is_abort.unwrap_or(false) {
            let tower_push = tower::abort_finish_push(ctx.state.db, player_id, active).await?;
            let (dungeon_update, end_dungeon) =
                dungeon::abort_dungeon_updates(ctx.state.db, player_id, active).await?;
            let refund = dungeon::settle_refund(ctx.state.db, player_id, active, false).await?;
            ctx.player_mut()?.battle.clear_active();
            send_refund(ctx, player_id, refund).await?;
            if let Some(tower_push) = tower_push {
                ctx.notify(CmdId::TowerBattleFinishPushCmd, tower_push)
                    .await?;
            }
            push::send_instruction_dungeon_info(ctx, player_id).await?;
            ctx.notify(CmdId::DungeonUpdatePushCmd, dungeon_update)
                .await?;
            ctx.notify(CmdId::DungeonEndDungeonPushCmd, end_dungeon)
                .await?;
            push::send_end_fight_push(ctx, dungeon::abort_end_fight(active)).await?;
        } else if active.is_victory() {
            let star = active.star();
            let round = active.current_round();
            let record =
                dungeon::prepare_dungeon_record(ctx.state.db, player_id, active, round).await?;
            let mut settlement = dungeon::settle_active(
                ctx.state.db,
                player_id,
                active,
                dungeon::DungeonCompletion {
                    star,
                    total_round: round,
                    multiplier: active.multiplication.unwrap_or(1).max(1),
                    fight_group: active.fight_group.as_ref(),
                },
                &record,
            )
            .await?;
            ctx.player_mut()?.battle.complete_active(record.pending);
            if let Some(compose) = settlement.compose_push.take() {
                ctx.notify(CmdId::TowerComposeFightSettlePushCmd, compose)
                    .await?;
            }
            send_completed_dungeon(
                ctx,
                player_id,
                active.chapter_id,
                active.episode_id,
                settlement,
            )
            .await?;
        } else {
            let mut refund = dungeon::settle_failed(ctx.state.db, player_id, active, true).await?;
            ctx.player_mut()?.battle.clear_active();
            if let Some(compose) = refund.compose_push.take() {
                ctx.notify(CmdId::TowerComposeFightSettlePushCmd, compose)
                    .await?;
            }
            send_refund(ctx, player_id, refund).await?;
            push::send_end_fight_push(ctx, dungeon::completed_end_fight(active)).await?;
        }
    }

    ctx.send_reply(
        CmdId::DungeonEndDungeonCmd,
        EndDungeonReply {},
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_cover_dungeon_record(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = CoverDungeonRecordRequest::decode(&req.data[..])?;
    let pending = ctx.player_mut()?.battle.take_pending_record();
    let is_cover = dungeon::cover_dungeon_record(
        ctx.state.db,
        player_id,
        pending,
        request.is_cover.unwrap_or(false),
    )
    .await?;

    ctx.send_reply(
        CmdId::CoverDungeonRecordCmd,
        CoverDungeonRecordReply {
            is_cover: Some(is_cover),
        },
        0,
        req.up_tag,
    )
    .await
}

async fn send_dungeon_settlement(
    ctx: &mut ConnectionContext,
    player_id: i64,
    settlement: dungeon::DungeonSettlement,
) -> Result<(), AppError> {
    push::send_hero_update_push(ctx, player_id, settlement.hero_ids).await?;
    if !settlement.open_infos.is_empty() {
        ctx.notify(
            CmdId::UpdateOpenPushCmd,
            UpdateOpenPush {
                open_infos: settlement.open_infos,
            },
        )
        .await?;
    }
    push::send_dungeon_completion_reward_pushes(ctx, player_id, settlement.rewards).await?;
    push::send_dungeon_map_progression(ctx, player_id).await?;
    push::send_instruction_dungeon_progression(ctx, player_id).await?;
    ctx.notify(CmdId::DungeonUpdatePushCmd, settlement.dungeon_update)
        .await?;
    ctx.notify(CmdId::DungeonEndDungeonPushCmd, settlement.end_dungeon)
        .await
}

pub(crate) async fn send_completed_dungeon(
    ctx: &mut ConnectionContext,
    player_id: i64,
    chapter_id: i32,
    episode_id: i32,
    settlement: dungeon::DungeonSettlement,
) -> Result<(), AppError> {
    notify_dungeon_pass_tasks(ctx, player_id, chapter_id).await?;
    send_dungeon_settlement(ctx, player_id, settlement).await?;
    task_events::notify(ctx, player_id, TaskEvent::EpisodeFinish { episode_id }).await
}

async fn notify_dungeon_pass_tasks(
    ctx: &mut ConnectionContext,
    player_id: i64,
    chapter_id: i32,
) -> Result<(), AppError> {
    for chapter_type in dungeon::dungeon_pass_types(chapter_id) {
        task_events::notify(
            ctx,
            player_id,
            TaskEvent::DungeonPass {
                chapter_type,
                count: 1,
            },
        )
        .await?;
    }
    task_events::notify(
        ctx,
        player_id,
        TaskEvent::DoneCount {
            name: "DungeonPass",
            count: 1,
        },
    )
    .await
}

async fn send_refund(
    ctx: &mut ConnectionContext,
    player_id: i64,
    settlement: dungeon::RefundSettlement,
) -> Result<(), AppError> {
    push::send_applied_reward_pushes(ctx, player_id, settlement.rewards, Vec::new(), None).await
}
