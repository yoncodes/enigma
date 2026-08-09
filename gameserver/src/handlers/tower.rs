use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    player::battle::ActiveBattle,
    tower,
    types::material_get_approach::MaterialGetApproach,
    util::{push, task_events},
};
use config::configs;
use logic::task::TaskEvent;
use prost::Message;
use sonettobuf::{
    CmdId, StartTowerBattleReply, StartTowerBattleRequest, TowerActiveTalentReply,
    TowerActiveTalentRequest, TowerChangeTalentPlanReply, TowerChangeTalentPlanRequest,
    TowerMopUpRequest, TowerRenameTalentPlanReply, TowerRenameTalentPlanRequest,
    TowerResetSubEpisodeRequest, TowerResetTalentReply, TowerResetTalentRequest,
};

pub async fn on_get_info(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let reply = tower::info(ctx.state.db, player_id).await?;
    ctx.send_reply(CmdId::GetTowerInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mop_up(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerMopUpRequest::decode(&req.data[..])?;
    let times = request.times.ok_or(AppError::InvalidRequest)?;
    let (reply, rewards, material_changes) = tower::mop_up(ctx.state.db, player_id, times).await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        rewards,
        material_changes,
        Some(MaterialGetApproach::Tower),
    )
    .await?;
    task_events::notify(ctx, player_id, TaskEvent::TowerMopUp { count: times }).await?;
    ctx.send_reply(CmdId::TowerMopUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_reset_sub_episode(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerResetSubEpisodeRequest::decode(&req.data[..])?;
    let reply = tower::reset_sub_episode(
        ctx.state.db,
        player_id,
        request.tower_type.ok_or(AppError::InvalidRequest)?,
        request.tower_id.ok_or(AppError::InvalidRequest)?,
        request.layer_id.ok_or(AppError::InvalidRequest)?,
        request.sub_episode.ok_or(AppError::InvalidRequest)?,
    )
    .await?;

    ctx.send_reply(CmdId::TowerResetSubEpisodeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_start_battle(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    ctx.player()?
        .battle
        .ensure_can_start(ctx.state.db, player_id)
        .await?;
    let msg = StartTowerBattleRequest::decode(&req.data[..])?;
    let (dungeon_request, tower_context) =
        crate::logic::battle_setup::tower::validate_battle_start(configs::get(), &msg)?;
    let episode_id = dungeon_request.episode_id.ok_or(AppError::InvalidRequest)?;
    let episode = configs::get()
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    if episode.battle_id == 0 {
        return Err(AppError::InvalidRequest);
    }

    let use_record = dungeon_request.use_record.unwrap_or(false);
    let fight_group = dungeon_request
        .fight_group
        .as_ref()
        .ok_or(AppError::InvalidRequest)?;
    let built = crate::logic::battle_setup::tower::build_fight(
        ctx.state.db,
        player_id,
        episode_id,
        episode.battle_id,
        fight_group,
        battle::dungeon::FightOptions {
            is_balance: dungeon_request.is_balance.unwrap_or(false),
            use_record,
        },
        tower_context,
    )
    .await?;
    let active = ActiveBattle::from_built(
        ctx.state.db,
        player_id,
        dungeon_request,
        built,
        Some(tower_context),
    )
    .await?;

    let start_dungeon_reply = active.start_reply();
    let cards = active.card_info_push();
    ctx.player_mut()?.battle.start_active(active);

    ctx.send_reply(
        CmdId::StartTowerBattleCmd,
        StartTowerBattleReply {
            start_dungeon_reply: Some(start_dungeon_reply),
            r#type: msg.r#type,
            tower_id: msg.tower_id,
            layer_id: msg.layer_id,
            difficulty: msg.difficulty,
            talent_plan_id: msg.talent_plan_id,
        },
        0,
        req.up_tag,
    )
    .await?;
    ctx.notify(CmdId::CardInfoPushCmd, cards).await
}

pub async fn on_active_talent(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerActiveTalentRequest::decode(&req.data[..])?;
    let boss_id = request.boss_id.ok_or(AppError::InvalidRequest)?;
    let talent_id = request.talent_id.ok_or(AppError::InvalidRequest)?;
    let talent_point = tower::activate_talent(ctx.state.db, player_id, boss_id, talent_id).await?;

    ctx.send_reply(
        CmdId::TowerActiveTalentCmd,
        TowerActiveTalentReply {
            boss_id: Some(boss_id),
            talent_id: Some(talent_id),
            talent_point: Some(talent_point),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_reset_talent(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerResetTalentRequest::decode(&req.data[..])?;
    let boss_id = request.boss_id.ok_or(AppError::InvalidRequest)?;
    let talent_id = request.talent_id.ok_or(AppError::InvalidRequest)?;
    let talent_point = tower::reset_talent(ctx.state.db, player_id, boss_id, talent_id).await?;

    ctx.send_reply(
        CmdId::TowerResetTalentCmd,
        TowerResetTalentReply {
            boss_id: Some(boss_id),
            talent_id: Some(talent_id),
            talent_point: Some(talent_point),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_change_talent_plan(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerChangeTalentPlanRequest::decode(&req.data[..])?;
    let boss_id = request.boss_id.ok_or(AppError::InvalidRequest)?;
    let plan_id = request.plan_id.ok_or(AppError::InvalidRequest)?;
    tower::change_talent_plan(ctx.state.db, player_id, boss_id, plan_id).await?;

    ctx.send_reply(
        CmdId::TowerChangeTalentPlanCmd,
        TowerChangeTalentPlanReply {
            boss_id: Some(boss_id),
            plan_id: Some(plan_id),
        },
        0,
        req.up_tag,
    )
    .await
}

pub async fn on_rename_talent_plan(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let request = TowerRenameTalentPlanRequest::decode(&req.data[..])?;
    let boss_id = request.boss_id.ok_or(AppError::InvalidRequest)?;
    let plan_name = request.plan_name.ok_or(AppError::InvalidRequest)?;
    tower::rename_active_talent_plan(ctx.state.db, player_id, boss_id, &plan_name).await?;

    ctx.send_reply(
        CmdId::TowerRenameTalentPlanCmd,
        TowerRenameTalentPlanReply {
            boss_id: Some(boss_id),
            plan_name: Some(plan_name.trim().to_owned()),
        },
        0,
        req.up_tag,
    )
    .await
}
