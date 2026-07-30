use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::{push, task_events},
};
use logic::task::TaskEvent;
use prost::Message;
use sonettobuf::{
    CancelHero3124TalentTreeRequest, ChoiceHero3123WeaponRequest, ChoiceHero3124TalentTreeRequest,
    CmdId, DestinyLevelUpRequest, DestinyRankUpRequest, DestinyStoneUnlockRequest,
    DestinyStoneUseRequest, GetHeroBirthdayRequest, HeroDefaultEquipRequest, HeroLevelUpRequest,
    HeroRankUpRequest, HeroRedDotReadRequest, HeroTouchRequest, HeroUpgradeSkillRequest,
    ItemUnlockRequest, MarkHeroFavorRequest, ResetHero3124TalentTreeRequest, UnMarkIsNewRequest,
    UnlockVoiceRequest, UseSkinRequest,
};

pub async fn on_unlock_voice(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = UnlockVoiceRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let reply = heroes
        .unlock_voice(
            ctx.state.db,
            hero_id,
            msg.voice_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    ctx.send_reply(CmdId::UnlockVoiceCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_item_unlock(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = ItemUnlockRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, reward) = heroes
        .unlock_item(
            ctx.state.db,
            hero_id,
            msg.item_id.ok_or(AppError::InvalidRequest)?,
        )
        .await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    push::send_currency_change_push(ctx, player_id, vec![reward]).await?;
    ctx.send_reply(CmdId::ItemUnlockCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_mark_hero_favor(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = MarkHeroFavorRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let is_favor = msg.is_favor.ok_or(AppError::InvalidRequest)?;
    let reply = heroes.mark_favor(ctx.state.db, hero_id, is_favor).await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    ctx.send_reply(CmdId::MarkHeroFavorCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_unmark_is_new(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = UnMarkIsNewRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let reply = heroes.unmark_new(ctx.state.db, hero_id).await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    ctx.send_reply(CmdId::UnMarkIsNewCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_use_skin(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = UseSkinRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let skin_id = msg.skin_id.ok_or(AppError::InvalidRequest)?;
    let reply = heroes.use_skin(ctx.state.db, hero_id, skin_id).await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    ctx.send_reply(CmdId::UseSkinCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_red_dot_read(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroRedDotReadRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let red_dot = msg.red_dot_type.ok_or(AppError::InvalidRequest)?;
    let reply = heroes.read_red_dot(ctx.state.db, hero_id, red_dot).await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    ctx.send_reply(CmdId::HeroRedDotReadCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_touch(ctx: &mut ConnectionContext, req: ClientPacket) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroTouchRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let reply = heroes.touch(ctx.state.db, hero_id).await?;

    push::send_hero_update_push(ctx, player_id, vec![hero_id]).await?;
    task_events::notify(
        ctx,
        player_id,
        TaskEvent::DoneCount {
            name: "HeroTouch",
            count: 1,
        },
    )
    .await?;
    ctx.send_reply(CmdId::HeroTouchCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_default_equip(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroDefaultEquipRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let equip_uid = msg.default_equip_uid.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes
        .default_equip(ctx.state.db, hero_id, equip_uid)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::HeroDefaultEquipCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_hero_birthday(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let heroes = ctx.player()?.hero;
    let player_id = ctx.player()?.id;
    let msg = GetHeroBirthdayRequest::decode(&req.data[..])?;
    let claim = heroes
        .birthday(ctx.state.db, msg.hero_id.ok_or(AppError::InvalidRequest)?)
        .await?;

    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(crate::types::material_get_approach::MaterialGetApproach::Birthday),
    )
    .await?;

    ctx.send_reply(CmdId::GetHeroBirthdayCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_choice_hero_3123_weapon(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = ChoiceHero3123WeaponRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let main_id = msg.main_id.ok_or(AppError::InvalidRequest)?;
    let sub_id = msg.sub_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes
        .choice_weapon(ctx.state.db, hero_id, main_id, sub_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::ChoiceHero3123WeaponCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_choice_hero_3124_talent_tree(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = ChoiceHero3124TalentTreeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let sub_id = msg.sub_id.ok_or(AppError::InvalidRequest)?;
    let level = msg.level.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes
        .choose_talent(ctx.state.db, hero_id, sub_id, level)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::ChoiceHero3124TalentTreeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_cancel_hero_3124_talent_tree(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = CancelHero3124TalentTreeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let sub_id = msg.sub_id.ok_or(AppError::InvalidRequest)?;
    let level = msg.level.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes
        .cancel_talent(ctx.state.db, hero_id, sub_id, level)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::CancelHero3124TalentTreeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_reset_hero_3124_talent_tree(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = ResetHero3124TalentTreeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes.reset_talents(ctx.state.db, hero_id).await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::ResetHero3124TalentTreeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_destiny_stone_use(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = DestinyStoneUseRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let stone_id = msg.stone_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = heroes
        .destiny_stone(ctx.state.db, hero_id, stone_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::DestinyStoneUseCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_destiny_rank_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = DestinyRankUpRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = heroes.destiny_rank_up(ctx.state.db, hero_id).await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        consumed.item_ids,
        consumed.currency_ids,
        consumed.material_changes,
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::DestinyRankUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_destiny_level_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = DestinyLevelUpRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let level = msg.level.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = heroes
        .destiny_level_up(ctx.state.db, hero_id, level)
        .await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        consumed.item_ids,
        consumed.currency_ids,
        consumed.material_changes,
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::DestinyLevelUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_destiny_stone_unlock(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = DestinyStoneUnlockRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let stone_id = msg.stone_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = heroes
        .destiny_stone_unlock(ctx.state.db, hero_id, stone_id)
        .await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        consumed.item_ids,
        consumed.currency_ids,
        consumed.material_changes,
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::DestinyStoneUnlockCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_level_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroLevelUpRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let new_level = msg.expect_level.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = heroes.level_up(ctx.state.db, hero_id, new_level).await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        consumed.item_ids,
        consumed.currency_ids,
        consumed.material_changes,
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    task_events::notify(
        ctx,
        player_id,
        TaskEvent::DoneCount {
            name: "HeroLevelUp",
            count: 1,
        },
    )
    .await?;
    ctx.send_reply(CmdId::HeroLevelUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_rank_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroRankUpRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = heroes.rank_up(ctx.state.db, hero_id).await?;

    push::send_cost_pushes(
        ctx,
        player_id,
        consumed.item_ids,
        consumed.currency_ids,
        consumed.material_changes,
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::HeroRankUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_upgrade_skill(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player = ctx.player()?;
    let player_id = player.id;
    let heroes = player.hero;
    let msg = HeroUpgradeSkillRequest::decode(&req.data[..])?;
    let (reply, hero_info, consumed_item_id) = heroes
        .upgrade_skill(
            ctx.state.db,
            msg.hero_id,
            msg.r#type,
            msg.consume.unwrap_or(1),
        )
        .await?;

    push::send_item_change_push(
        ctx,
        player_id,
        vec![consumed_item_id],
        Vec::new(),
        Vec::new(),
    )
    .await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::HeroUpgradeSkillCmd, reply, 0, req.up_tag)
        .await
}
