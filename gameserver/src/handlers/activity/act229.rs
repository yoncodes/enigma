use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    player::battle::{Act229BattleContext, ActiveBattle},
};
use config::configs;
use prost::Message;
use sonettobuf::{
    Act229ResetStageRequest, CmdId, GetAct229InfoRequest, StartAct229BattleReply,
    StartAct229BattleRequest,
};

pub async fn on_get_act229_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct229InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act229_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct229InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_start_act229_battle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    ctx.player()?
        .battle
        .ensure_can_start(ctx.state.db, player_id)
        .await?;
    let msg = StartAct229BattleRequest::decode(&req.data[..])?;
    let activity_id = msg.activity_id.ok_or(AppError::InvalidRequest)?;
    let stage_id = msg.stage_id.ok_or(AppError::InvalidRequest)?;
    let dungeon_request = msg.start_dungeon_request.ok_or(AppError::InvalidRequest)?;
    let episode_id = ctx
        .player()?
        .activity
        .act229_battle_episode(activity_id, stage_id)?;
    let episode = configs::get()
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    if dungeon_request.episode_id != Some(episode_id)
        || dungeon_request
            .chapter_id
            .is_some_and(|chapter_id| chapter_id != episode.chapter_id)
        || episode.battle_id <= 0
    {
        return Err(AppError::InvalidRequest);
    }

    let fight_group = dungeon_request
        .fight_group
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    let built = battle::dungeon::build_fight(
        ctx.state.db,
        player_id,
        episode_id,
        episode.battle_id,
        fight_group,
        battle::dungeon::FightOptions {
            is_balance: dungeon_request.is_balance.unwrap_or(false),
            use_record: dungeon_request.use_record.unwrap_or(false),
        },
        dungeon_request.params.as_deref(),
    )
    .await?;
    let mut active = ActiveBattle::prepare_act229(
        dungeon_request,
        built,
        Act229BattleContext {
            activity_id,
            stage_id,
        },
    )?;
    let heroes = active.act229_heroes();
    ctx.player()?
        .activity
        .ensure_act229_heroes_available(ctx.state.db, activity_id, stage_id, &heroes)
        .await?;
    active
        .activate(ctx.state.db, player_id, &Default::default())
        .await?;
    let start_dungeon_reply = active.start_reply();
    let cards = active.card_info_push();
    ctx.player_mut()?.battle.start_active(active);

    ctx.send_reply(
        CmdId::StartAct229BattleCmd,
        StartAct229BattleReply {
            start_dungeon_reply: Some(start_dungeon_reply),
            activity_id: Some(activity_id),
            stage_id: Some(stage_id),
        },
        0,
        req.up_tag,
    )
    .await?;
    ctx.notify(CmdId::CardInfoPushCmd, cards).await
}

pub async fn on_reset_act229_stage(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Act229ResetStageRequest::decode(&req.data[..])?;
    let activity_id = msg.activity_id.ok_or(AppError::InvalidRequest)?;
    let stage_id = msg.stage_id.ok_or(AppError::InvalidRequest)?;
    let db = ctx.state.db;
    let reply = ctx
        .player()?
        .activity
        .reset_act229_stage(db, activity_id, stage_id)
        .await?;

    ctx.send_reply(CmdId::Act229ResetStageCmd, reply, 0, req.up_tag)
        .await
}
