use crate::{
    error::AppError, logic::reward::AppliedRewards, net::context::ConnectionContext,
    types::material_get_approach::MaterialGetApproach, util::task_events,
};
use logic::task::TaskEvent;
use sonettobuf::{
    AntiqueInfo, AntiqueUpdatePush, BlockPackageGainPush, BpScoreUpdatePush, BuildingGainPush,
    ChapterMapElementUpdatePush, ChapterMapUpdatePush, ClothUpdatePush, CmdId, CurrencyChangePush,
    EndFightPush, EquipUpdatePush, GainSpecialBlockPush, HeroSkinGainPush, HeroUpdatePush,
    ItemChangePush, MaterialChangePush, MaterialData, PlayerCardInfoPush, PlayerCloth,
    PlayerClothInfo, RedDotGroup, RedDotInfo, UpdateRedDotPush,
};
use std::collections::HashMap;

pub async fn send_end_fight_push(
    ctx: &mut ConnectionContext,
    push: EndFightPush,
) -> Result<(), AppError> {
    ctx.notify(CmdId::FightEndFightPushCmd, push).await
}

pub async fn send_dungeon_map_progression(
    ctx: &mut ConnectionContext,
    player_id: i64,
) -> Result<(), AppError> {
    let progression = crate::dungeon::reconcile_map_progression(ctx.state.db, player_id).await?;
    if !progression.map_ids.is_empty() {
        ctx.notify(
            CmdId::ChapterMapUpdatePushCmd,
            ChapterMapUpdatePush {
                map_ids: progression.map_ids,
            },
        )
        .await?;
    }
    if !progression.elements.is_empty() {
        ctx.notify(
            CmdId::ChapterMapElementUpdatePushCmd,
            ChapterMapElementUpdatePush {
                elements: progression.elements,
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn send_instruction_dungeon_progression(
    ctx: &mut ConnectionContext,
    player_id: i64,
) -> Result<(), AppError> {
    if let Some(info) =
        crate::dungeon::reconcile_instruction_dungeon(ctx.state.db, player_id).await?
    {
        ctx.notify(CmdId::DungeonInstructionDungeonInfoPushCmd, info)
            .await?;
    }
    Ok(())
}

pub async fn send_instruction_dungeon_info(
    ctx: &mut ConnectionContext,
    player_id: i64,
) -> Result<(), AppError> {
    let info = crate::dungeon::instruction_dungeon_push(ctx.state.db, player_id).await?;
    ctx.notify(CmdId::DungeonInstructionDungeonInfoPushCmd, info)
        .await
}

pub async fn send_currency_change_push(
    ctx: &mut ConnectionContext,
    player_id: i64,
    changes: Vec<(i32, i32)>,
) -> Result<(), AppError> {
    if changes.is_empty() {
        return Ok(());
    }

    let mut totals = HashMap::<i32, i32>::new();
    for (currency_id, amount) in changes {
        *totals.entry(currency_id).or_default() += amount;
    }

    let change_currency = crate::logic::inventory::InventoryManager::new(player_id)
        .currency_snapshots(ctx.state.db, totals.keys().copied())
        .await?;

    if !change_currency.is_empty() {
        ctx.notify(
            CmdId::CurrencyChangePushCmd,
            CurrencyChangePush { change_currency },
        )
        .await?;
    }

    for (currency_id, amount) in totals {
        if amount < 0 {
            task_events::notify(
                ctx,
                player_id,
                TaskEvent::CurrencyDec {
                    currency_id,
                    amount: -amount,
                },
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn send_material_change_push(
    ctx: &mut ConnectionContext,
    rewards: Vec<(u32, u32, i32)>,
    get_approach: Option<MaterialGetApproach>,
) -> Result<(), AppError> {
    if rewards.is_empty() {
        return Ok(());
    }

    let data_list = rewards
        .iter()
        .map(|(materil_type, materil_id, quantity)| MaterialData {
            materil_type: Some(*materil_type),
            materil_id: Some(*materil_id),
            quantity: Some(*quantity),
        })
        .collect();

    ctx.notify(
        CmdId::MaterialChangePushCmd,
        MaterialChangePush {
            data_list,
            get_approach: get_approach.map(MaterialGetApproach::id),
        },
    )
    .await
}

pub async fn send_applied_reward_pushes(
    ctx: &mut ConnectionContext,
    player_id: i64,
    rewards: AppliedRewards,
    material_changes: Vec<(u32, u32, i32)>,
    get_approach: Option<MaterialGetApproach>,
) -> Result<(), AppError> {
    send_reward_pushes(
        ctx,
        player_id,
        rewards,
        material_changes,
        get_approach,
        RewardFlow::Claim,
    )
    .await
}

pub async fn send_item_first_applied_reward_pushes(
    ctx: &mut ConnectionContext,
    player_id: i64,
    rewards: AppliedRewards,
    material_changes: Vec<(u32, u32, i32)>,
    get_approach: Option<MaterialGetApproach>,
) -> Result<(), AppError> {
    send_reward_pushes(
        ctx,
        player_id,
        rewards,
        material_changes,
        get_approach,
        RewardFlow::ItemFirst,
    )
    .await
}

pub async fn send_dungeon_completion_reward_pushes(
    ctx: &mut ConnectionContext,
    player_id: i64,
    rewards: AppliedRewards,
) -> Result<(), AppError> {
    send_reward_pushes(
        ctx,
        player_id,
        rewards,
        Vec::new(),
        None,
        RewardFlow::DungeonCompletion,
    )
    .await
}

#[derive(Clone, Copy)]
enum RewardFlow {
    Claim,
    DungeonCompletion,
    ItemFirst,
}

async fn send_reward_pushes(
    ctx: &mut ConnectionContext,
    player_id: i64,
    rewards: AppliedRewards,
    material_changes: Vec<(u32, u32, i32)>,
    get_approach: Option<MaterialGetApproach>,
    flow: RewardFlow,
) -> Result<(), AppError> {
    let item_changed = !rewards.item_ids.is_empty()
        || !rewards.power_item_ids.is_empty()
        || !rewards.insight_item_ids.is_empty();
    send_room_reward_pushes(ctx, &rewards).await?;
    match flow {
        RewardFlow::Claim => {
            send_currency_change_push(ctx, player_id, rewards.currency_ids).await?;
            send_equip_update_push(ctx, player_id, rewards.equip_uids).await?;
        }
        RewardFlow::DungeonCompletion => {
            send_equip_update_push(ctx, player_id, rewards.equip_uids).await?;
            send_currency_change_push(ctx, player_id, rewards.currency_ids).await?;
        }
        RewardFlow::ItemFirst => {
            send_item_change_push(
                ctx,
                player_id,
                rewards.item_ids.clone(),
                rewards.power_item_ids.clone(),
                rewards.insight_item_ids.clone(),
            )
            .await?;
            if item_changed {
                send_trade_order_red_dot(ctx).await?;
            }
            send_currency_change_push(ctx, player_id, rewards.currency_ids).await?;
            send_equip_update_push(ctx, player_id, rewards.equip_uids).await?;
        }
    }
    if !matches!(flow, RewardFlow::ItemFirst) {
        send_item_change_push(
            ctx,
            player_id,
            rewards.item_ids,
            rewards.power_item_ids,
            rewards.insight_item_ids,
        )
        .await?;
        if item_changed {
            send_trade_order_red_dot(ctx).await?;
        }
    }
    send_hero_update_push(ctx, player_id, rewards.hero_ids).await?;
    send_antique_update_push(ctx, rewards.antiques).await?;
    send_skin_gain_pushes(ctx, &rewards.skin_gains, get_approach).await?;
    send_cloth_update_push(ctx, rewards.cloth_updates).await?;
    send_bp_score_update_pushes(ctx, &rewards.bp_scores).await?;
    if rewards.player_info_changed {
        ctx.notify(
            CmdId::PlayerInfoPushCmd,
            crate::logic::profile::ProfileManager::new(player_id)
                .snapshot(ctx.state.db)
                .await?,
        )
        .await?;
    }
    send_material_change_push(ctx, material_changes, get_approach).await
}

pub async fn send_trade_order_red_dot(ctx: &mut ConnectionContext) -> Result<(), AppError> {
    let value = ctx
        .player()?
        .red_dot
        .trade_order_value(ctx.state.db)
        .await?;
    ctx.push_red_dot_value(
        crate::types::red_dot_id::RedDotId::TradeOrderFulfillable.id(),
        vec![0],
        true,
        value,
        0,
    )
    .await
}

pub async fn send_room_reward_pushes(
    ctx: &mut ConnectionContext,
    rewards: &AppliedRewards,
) -> Result<(), AppError> {
    if !rewards.block_packages.is_empty() {
        ctx.notify(
            CmdId::BlockPackageGainPushCmd,
            BlockPackageGainPush {
                block_packages: rewards
                    .block_packages
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            },
        )
        .await?;
    }
    if !rewards.room_buildings.is_empty() {
        ctx.notify(
            CmdId::BuildingGainPushCmd,
            BuildingGainPush {
                building_infos: rewards
                    .room_buildings
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            },
        )
        .await?;
    }
    if !rewards.special_blocks.is_empty() {
        ctx.notify(
            CmdId::GainSpecialBlockPushCmd,
            GainSpecialBlockPush {
                special_blocks: rewards
                    .special_blocks
                    .iter()
                    .map(|block| block.block_id)
                    .collect(),
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn send_cost_pushes(
    ctx: &mut ConnectionContext,
    player_id: i64,
    item_ids: Vec<u32>,
    currency_ids: Vec<(i32, i32)>,
    material_changes: Vec<(u32, u32, i32)>,
) -> Result<(), AppError> {
    send_item_change_push(ctx, player_id, item_ids, Vec::new(), Vec::new()).await?;
    send_currency_change_push(ctx, player_id, currency_ids).await?;
    send_material_change_push(ctx, material_changes, None).await
}

pub async fn send_cloth_update_push(
    ctx: &mut ConnectionContext,
    clothes: Vec<PlayerCloth>,
) -> Result<(), AppError> {
    if clothes.is_empty() {
        return Ok(());
    }

    ctx.notify(
        CmdId::ClothUpdatePushCmd,
        ClothUpdatePush {
            update_infos: Some(PlayerClothInfo { clothes }),
        },
    )
    .await
}

pub async fn send_antique_update_push(
    ctx: &mut ConnectionContext,
    antiques: Vec<AntiqueInfo>,
) -> Result<(), AppError> {
    if antiques.is_empty() {
        return Ok(());
    }

    ctx.notify(CmdId::AntiqueUpdatePushCmd, AntiqueUpdatePush { antiques })
        .await
}

pub async fn send_bp_score_update_pushes(
    ctx: &mut ConnectionContext,
    scores: &[crate::logic::reward::BpScoreGain],
) -> Result<(), AppError> {
    for score in scores {
        ctx.notify(
            CmdId::BpScoreUpdatePushCmd,
            BpScoreUpdatePush {
                id: Some(score.bp_id),
                score: Some(score.score),
                weekly_score: Some(score.weekly_score),
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn send_skin_gain_pushes(
    ctx: &mut ConnectionContext,
    skins: &[crate::logic::reward::SkinGain],
    get_approach: Option<MaterialGetApproach>,
) -> Result<(), AppError> {
    for skin in skins {
        ctx.notify(
            CmdId::HeroSkinGainPushCmd,
            HeroSkinGainPush {
                skin_id: Some(skin.skin_id),
                first_gain: Some(skin.first_gain),
                get_approach: get_approach.map(|approach| approach.id() as i32),
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn send_red_dot_push(
    ctx: &mut ConnectionContext,
    define_id: i32,
    info_ids: Vec<i32>,
    replace_all: bool,
) -> Result<(), AppError> {
    send_red_dot_value_push(ctx, define_id, info_ids, replace_all, 0, 0).await
}

pub async fn send_red_dot_groups(
    ctx: &mut ConnectionContext,
    groups: Vec<RedDotGroup>,
) -> Result<(), AppError> {
    if groups.is_empty() {
        return Ok(());
    }

    ctx.notify(
        CmdId::UpdateRedDotPushCmd,
        UpdateRedDotPush {
            red_dot_infos: groups,
            replace_all: None,
        },
    )
    .await
}

pub async fn clear_red_dot_infos(
    ctx: &mut ConnectionContext,
    define_id: i32,
) -> Result<(), AppError> {
    send_red_dot_groups(
        ctx,
        vec![RedDotGroup {
            define_id,
            infos: Vec::new(),
            replace_all: Some(true),
        }],
    )
    .await
}

pub async fn clear_red_dots(
    ctx: &mut ConnectionContext,
    define_ids: impl IntoIterator<Item = i32>,
) -> Result<(), AppError> {
    send_red_dot_groups(
        ctx,
        define_ids
            .into_iter()
            .map(|define_id| RedDotGroup {
                define_id,
                infos: vec![RedDotInfo {
                    id: 0,
                    value: 0,
                    time: Some(0),
                    ext: None,
                }],
                replace_all: Some(true),
            })
            .collect(),
    )
    .await
}

pub async fn send_red_dot_value_push(
    ctx: &mut ConnectionContext,
    define_id: i32,
    info_ids: Vec<i32>,
    replace_all: bool,
    value: i32,
    time: i32,
) -> Result<(), AppError> {
    if info_ids.is_empty() {
        return Ok(());
    }

    let group = RedDotGroup {
        define_id,
        infos: info_ids
            .iter()
            .map(|id| RedDotInfo {
                id: *id as i64,
                value,
                time: Some(time),
                ext: None,
            })
            .collect(),
        replace_all: Some(replace_all),
    };

    send_red_dot_groups(ctx, vec![group]).await
}

pub async fn send_item_change_push(
    ctx: &mut ConnectionContext,
    player_id: i64,
    item_ids: Vec<u32>,
    power_item_ids: Vec<i32>,
    insight_item_ids: Vec<i32>,
) -> Result<(), AppError> {
    if item_ids.is_empty() && power_item_ids.is_empty() && insight_item_ids.is_empty() {
        return Ok(());
    }

    let changed = crate::logic::inventory::InventoryManager::new(player_id)
        .item_snapshots(ctx.state.db, item_ids, power_item_ids, insight_item_ids)
        .await?;

    ctx.notify(
        CmdId::ItemChangePushCmd,
        ItemChangePush {
            items: changed.items,
            power_items: changed.power_items,
            insight_items: changed.insight_items,
            expire_items: Vec::new(),
            talent_items: Vec::new(),
        },
    )
    .await
}

pub async fn send_equip_update_push(
    ctx: &mut ConnectionContext,
    player_id: i64,
    equip_uids: Vec<i64>,
) -> Result<(), AppError> {
    if equip_uids.is_empty() {
        return Ok(());
    }

    let equips = crate::logic::inventory::InventoryManager::new(player_id)
        .equipment_snapshots(ctx.state.db, equip_uids)
        .await?;

    ctx.notify(CmdId::EquipUpdatePushCmd, EquipUpdatePush { equips })
        .await
}

pub async fn send_hero_update_push(
    ctx: &mut ConnectionContext,
    player_id: i64,
    hero_ids: Vec<i32>,
) -> Result<(), AppError> {
    if hero_ids.is_empty() {
        return Ok(());
    }

    let hero_updates = crate::logic::hero::HeroManager::new(player_id)
        .snapshots(ctx.state.db, hero_ids)
        .await?;

    send_hero_updates(ctx, player_id, hero_updates).await
}

pub async fn send_hero_updates(
    ctx: &mut ConnectionContext,
    player_id: i64,
    hero_updates: Vec<sonettobuf::HeroInfo>,
) -> Result<(), AppError> {
    ctx.notify(
        CmdId::HeroHeroUpdatePushCmd,
        HeroUpdatePush { hero_updates },
    )
    .await?;
    send_player_card_info_push(ctx, player_id).await
}

pub async fn send_player_card_info_push(
    ctx: &mut ConnectionContext,
    player_id: i64,
) -> Result<(), AppError> {
    let reply = crate::logic::profile::ProfileManager::new(player_id)
        .card_info(ctx.state.db)
        .await?;
    ctx.notify(
        CmdId::PlayerCardInfoPushCmd,
        PlayerCardInfoPush {
            player_card_info: reply.player_card_info,
        },
    )
    .await
}
