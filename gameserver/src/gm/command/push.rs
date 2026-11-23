use std::collections::HashSet;

use anyhow::Result;
use database::db::game::{block_packages, currencies, equipment, items};
use sonettobuf::{
    BlockPackageGainPush, CmdId, CurrencyChangePush, EquipUpdatePush, HeroSkinGainPush,
    HeroUpdatePush, ItemChangePush, MaterialChangePush, MaterialData, PlayerCardInfoPush,
    PlayerClothInfo, prost::Message,
};

use crate::{
    logic::reward,
    net::{app::AppState, outbound::CommandPacket},
};

use super::{
    super::catalog::MaterialKind,
    grant::{GrantData, RewardData},
};

pub(super) async fn send_material_push(
    state: &'static AppState,
    player_id: i64,
    rewards: impl IntoIterator<Item = RewardData>,
) -> Result<()> {
    let data_list = rewards
        .into_iter()
        .filter_map(|reward| {
            let kind = reward::RewardMaterialType::from_i32(reward.r#type)?;
            Some(MaterialData {
                materil_type: Some(kind.id()),
                materil_id: Some(reward.id as u32),
                quantity: Some(reward.amount),
            })
        })
        .collect::<Vec<_>>();
    if !data_list.is_empty() {
        send_push(
            state,
            player_id,
            CmdId::MaterialChangePushCmd,
            MaterialChangePush {
                data_list,
                get_approach: None,
            },
        )
        .await?;
    }
    Ok(())
}

pub(in crate::gm) async fn send_snapshot_pushes(
    state: &'static AppState,
    data: &GrantData,
) -> Result<()> {
    let mut changed_items = Vec::new();
    for item_id in &data.changed_item_ids {
        if let Some(item) = items::get_item(state.db, data.user_id, *item_id as u32).await? {
            changed_items.push(item.into());
        }
    }

    let mut changed_power_items = Vec::new();
    for item_id in &data.changed_power_item_ids {
        if let Some(item) = items::get_power_item(state.db, data.user_id, *item_id as u32).await? {
            changed_power_items.push(item.into());
        }
    }

    let mut changed_insight_items = Vec::new();
    for item_id in &data.changed_insight_item_ids {
        if let Some(item) = items::get_insight_item(state.db, data.user_id, *item_id as u32).await?
        {
            changed_insight_items.push(item.into());
        }
    }

    send_item_push(
        state,
        data.user_id,
        changed_items,
        changed_power_items,
        changed_insight_items,
    )
    .await?;

    let mut block_package_ids = HashSet::new();
    for reward in data
        .rewards
        .iter()
        .filter(|reward| MaterialKind::from_raw(reward.r#type) == Some(MaterialKind::BlockPackage))
        .filter(|reward| block_package_ids.insert(reward.id))
    {
        let packages = block_packages::get_block_packages(state.db, data.user_id)
            .await?
            .into_iter()
            .filter(|package| package.block_package_id == reward.id)
            .map(Into::into)
            .collect();
        send_push(
            state,
            data.user_id,
            CmdId::BlockPackageGainPushCmd,
            BlockPackageGainPush {
                block_packages: packages,
            },
        )
        .await?;
    }

    let mut change_currency = Vec::new();
    for currency_id in &data.changed_currency_ids {
        if let Some(currency) =
            currencies::get_currency(state.db, data.user_id, *currency_id).await?
        {
            change_currency.push(currency.into());
        }
    }

    if !change_currency.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::CurrencyChangePushCmd,
            CurrencyChangePush { change_currency },
        )
        .await?;
    }

    let mut equips = Vec::new();
    for equip_uid in &data.changed_equip_ids {
        equips.push(
            equipment::get_equipment_by_uid(state.db, data.user_id, *equip_uid)
                .await?
                .into(),
        );
    }

    if !equips.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::EquipUpdatePushCmd,
            EquipUpdatePush { equips },
        )
        .await?;
    }

    let hero_updates = crate::logic::hero::HeroManager::new(data.user_id)
        .snapshots(state.db, data.changed_hero_ids.iter().copied())
        .await?;

    if !hero_updates.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::HeroHeroUpdatePushCmd,
            HeroUpdatePush { hero_updates },
        )
        .await?;
        let player_card = crate::logic::profile::ProfileManager::new(data.user_id)
            .card_info(state.db)
            .await?;
        send_push(
            state,
            data.user_id,
            CmdId::PlayerCardInfoPushCmd,
            PlayerCardInfoPush {
                player_card_info: player_card.player_card_info,
            },
        )
        .await?;
    }

    for skin_id in &data.changed_skin_ids {
        send_push(
            state,
            data.user_id,
            CmdId::HeroSkinGainPushCmd,
            HeroSkinGainPush {
                skin_id: Some(*skin_id),
                first_gain: Some(true),
                get_approach: None,
            },
        )
        .await?;
    }

    if data.player_info_changed {
        send_push(
            state,
            data.user_id,
            CmdId::PlayerInfoPushCmd,
            crate::logic::profile::ProfileManager::new(data.user_id)
                .snapshot(state.db)
                .await?,
        )
        .await?;
    }
    if !data.cloth_updates.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::ClothUpdatePushCmd,
            sonettobuf::ClothUpdatePush {
                update_infos: Some(PlayerClothInfo {
                    clothes: data.cloth_updates.clone(),
                }),
            },
        )
        .await?;
    }

    Ok(())
}

async fn send_item_push(
    state: &'static AppState,
    player_id: i64,
    items: Vec<sonettobuf::Item>,
    power_items: Vec<sonettobuf::PowerItem>,
    insight_items: Vec<sonettobuf::InsightItem>,
) -> Result<()> {
    if items.is_empty() && power_items.is_empty() && insight_items.is_empty() {
        return Ok(());
    }

    send_push(
        state,
        player_id,
        CmdId::ItemChangePushCmd,
        ItemChangePush {
            items,
            power_items,
            insight_items,
            expire_items: Vec::new(),
            talent_items: Vec::new(),
        },
    )
    .await
}

pub(in crate::gm) async fn send_push<M: Message>(
    state: &'static AppState,
    player_id: i64,
    cmd_id: CmdId,
    message: M,
) -> Result<()> {
    let Some(sender) = state.get_session_sender(player_id) else {
        return Ok(());
    };

    let down_tag = state.reserve_down_tag().await;
    sender
        .send(CommandPacket::Push {
            cmd_id,
            body: message.encode_to_vec(),
            down_tag,
        })
        .await
        .map_err(|err| anyhow::anyhow!("failed to send MUIP push: {err}"))
}
