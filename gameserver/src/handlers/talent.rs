use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
    util::push,
};
use prost::Message;
use sonettobuf::{
    CmdId, HeroTalentStyleStatRequest, HeroTalentUpRequest, PutTalentCubeBatchRequest,
    PutTalentCubeRequest, PutTalentSchemeRequest, RenameTalentTemplateRequest,
    TakeoffAllTalentCubeRequest, TalentCubeInfo, TalentStyleReadRequest, UnlockTalentStyleRequest,
    UseTalentStyleRequest, UseTalentTemplateRequest,
};

pub async fn on_talent_style_read(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TalentStyleReadRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .talent_style_read(ctx.state.db, hero_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::TalentStyleReadCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_put_talent_cube(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = PutTalentCubeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .put_talent_cube(
            ctx.state.db,
            hero_id,
            template_id,
            cube_pos(msg.get_cube_info),
            cube_full(msg.put_cube_info),
        )
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::PutTalentCubeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_put_talent_cube_batch(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = PutTalentCubeBatchRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let cubes = msg
        .put_cube_info
        .into_iter()
        .map(cube_full_required)
        .collect::<Result<Vec<_>, _>>()?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .put_talent_cube_batch(ctx.state.db, hero_id, template_id, msg.style, cubes)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::PutTalentCubeBatchCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_takeoff_all_talent_cube(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = TakeoffAllTalentCubeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .takeoff_all_talent_cubes(ctx.state.db, hero_id, template_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::TakeoffAllTalentCubeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_rename_talent_template(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = RenameTalentTemplateRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .hero
        .rename_talent_template(
            ctx.state.db,
            msg.hero_id.ok_or(AppError::InvalidRequest)?,
            msg.template_id.ok_or(AppError::InvalidRequest)?,
            msg.name.ok_or(AppError::InvalidRequest)?,
        )
        .await?;
    ctx.send_reply(CmdId::RenameTalentTemplateCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_talent_up(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = HeroTalentUpRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = ctx.player()?.hero.talent_up(ctx.state.db, hero_id).await?;

    push::send_cost_pushes(ctx, player_id, consumed.item_ids, consumed.currency_ids).await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::HeroTalentUpCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_put_talent_scheme(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = PutTalentSchemeRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let talent_id = msg.talent_id.ok_or(AppError::InvalidRequest)?;
    let talent_mould = msg.talent_mould.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .put_talent_scheme(ctx.state.db, hero_id, talent_id, talent_mould, template_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::PutTalentSchemeCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_hero_talent_style_stat(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = HeroTalentStyleStatRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let reply = ctx.player()?.hero.talent_style_stat(hero_id);

    ctx.send_reply(CmdId::HeroTalentStyleStatCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_unlock_talent_style(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UnlockTalentStyleRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let style = msg.style.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info, consumed) = ctx
        .player()?
        .hero
        .unlock_talent_style(ctx.state.db, hero_id, style)
        .await?;

    push::send_cost_pushes(ctx, player_id, consumed.item_ids, consumed.currency_ids).await?;
    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::UnlockTalentStyleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_use_talent_style(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UseTalentStyleRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let style = msg.style.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .use_talent_style(ctx.state.db, hero_id, template_id, style)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::UseTalentStyleCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_use_talent_template(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let player_id = ctx.player()?.id;
    let msg = UseTalentTemplateRequest::decode(&req.data[..])?;
    let hero_id = msg.hero_id.ok_or(AppError::InvalidRequest)?;
    let template_id = msg.template_id.ok_or(AppError::InvalidRequest)?;
    let (reply, hero_info) = ctx
        .player()?
        .hero
        .use_talent_template(ctx.state.db, hero_id, template_id)
        .await?;

    push::send_hero_updates(ctx, player_id, vec![hero_info]).await?;
    ctx.send_reply(CmdId::UseTalentTemplateCmd, reply, 0, req.up_tag)
        .await
}

fn cube_pos(cube: Option<TalentCubeInfo>) -> Option<(i32, i32)> {
    let cube = cube?;
    Some((cube.pos_x?, cube.pos_y?))
}

fn cube_full(cube: Option<TalentCubeInfo>) -> Option<(i32, i32, i32, i32)> {
    let cube = cube?;
    Some((cube.cube_id?, cube.direction?, cube.pos_x?, cube.pos_y?))
}

fn cube_full_required(cube: TalentCubeInfo) -> Result<(i32, i32, i32, i32), AppError> {
    Ok((
        cube.cube_id.ok_or(AppError::InvalidRequest)?,
        cube.direction.ok_or(AppError::InvalidRequest)?,
        cube.pos_x.ok_or(AppError::InvalidRequest)?,
        cube.pos_y.ok_or(AppError::InvalidRequest)?,
    ))
}
