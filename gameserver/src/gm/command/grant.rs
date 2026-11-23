use std::collections::HashSet;

use anyhow::Result;
use database::{
    db::game::{block_packages, equipment},
    models::game::{currencies::UserCurrencyModel, heros::UserHeroModel, items::UserItemModel},
};
use muipserver::GmResponse;
use serde::Serialize;
use sonettobuf::PlayerCloth;

use crate::{logic::reward, net::app::AppState};

use super::{
    super::catalog::MaterialKind,
    push::{send_material_push, send_snapshot_pushes},
};

pub(super) async fn grant(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    if args.len() != 3 {
        anyhow::bail!(
            "usage: material <type> <id> <amount> or give <item|currency|hero|skin|equip|power|package|insight> <id> <amount>"
        );
    }

    let kind = MaterialKind::parse(args[0])
        .ok_or_else(|| anyhow::anyhow!("unknown material type '{}'", args[0]))?;
    let id = parse_positive(args[1], "material id")?;
    let amount = parse_positive(args[2], "amount")?;

    let mut data = GrantData {
        user_id: player_id,
        rewards: vec![RewardData {
            r#type: kind.id(),
            id,
            amount,
        }],
        changed_item_ids: Vec::new(),
        changed_power_item_ids: Vec::new(),
        changed_insight_item_ids: Vec::new(),
        changed_currency_ids: Vec::new(),
        changed_hero_ids: Vec::new(),
        changed_skin_ids: Vec::new(),
        changed_equip_ids: Vec::new(),
        cloth_updates: Vec::new(),
        player_info_changed: false,
    };

    match kind {
        MaterialKind::Item => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_item_ids = model.create_items(vec![(id as u32, amount)]).await?;
        }
        MaterialKind::Currency => {
            let model = UserCurrencyModel::new(player_id, (*state.db).clone());
            data.changed_currency_ids = model
                .create_currencies(&[(id, amount)])
                .await?
                .into_iter()
                .map(|(id, _)| id)
                .collect();
        }
        MaterialKind::PlayerExp => {
            let applied = reward::RewardManager::new(player_id)
                .apply(
                    state.db,
                    reward::RewardSet {
                        player_exp: amount,
                        ..Default::default()
                    },
                )
                .await?;
            data.merge_rewards(applied);
        }
        MaterialKind::Hero => {
            let model = UserHeroModel::new(player_id, (*state.db).clone());
            for _ in 0..amount {
                if model.has_hero(id).await? {
                    let duplicate_count = model.add_hero_duplicate(id).await?;
                    let rewards = reward::hero_duplicate_rewards(id, duplicate_count)?;
                    let applied = reward::RewardManager::new(player_id)
                        .apply(state.db, rewards)
                        .await?;
                    data.merge_rewards(applied);
                } else {
                    model.create_hero(id).await?;
                }
            }
            data.changed_hero_ids.push(id);
        }
        MaterialKind::Skin => {
            let model = UserHeroModel::new(player_id, (*state.db).clone());
            if model.unlock_skin(id).await? {
                data.changed_skin_ids.push(id);
            }
        }
        MaterialKind::Equipment => {
            data.changed_equip_ids =
                equipment::add_equipments(state.db, player_id, &[(id, amount)]).await?;
        }
        MaterialKind::PowerItem => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_power_item_ids = model.create_power_items(vec![(id, amount)]).await?;
        }
        MaterialKind::BlockPackage => {
            block_packages::add_block_package(state.db, player_id, id).await?;
        }
        MaterialKind::InsightItem => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_insight_item_ids = model.create_insight_items(vec![(id, amount)]).await?;
        }
    }

    send_material_push(state, data.user_id, data.rewards.iter().cloned()).await?;
    send_snapshot_pushes(state, &data).await?;

    Ok(GmResponse::ok_data(
        format!("granted {amount} of {}#{id} to {player_id}", kind.id()),
        data,
    ))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::gm) struct GrantData {
    pub(in crate::gm) user_id: i64,
    pub(super) rewards: Vec<RewardData>,
    pub(in crate::gm) changed_item_ids: Vec<i32>,
    pub(in crate::gm) changed_power_item_ids: Vec<i32>,
    pub(in crate::gm) changed_insight_item_ids: Vec<i32>,
    pub(in crate::gm) changed_currency_ids: Vec<i32>,
    pub(in crate::gm) changed_hero_ids: Vec<i32>,
    pub(in crate::gm) changed_skin_ids: Vec<i32>,
    pub(in crate::gm) changed_equip_ids: Vec<i64>,
    pub(in crate::gm) cloth_updates: Vec<PlayerCloth>,
    pub(in crate::gm) player_info_changed: bool,
}

impl GrantData {
    pub(in crate::gm) fn empty(user_id: i64) -> Self {
        Self {
            user_id,
            rewards: Vec::new(),
            changed_item_ids: Vec::new(),
            changed_power_item_ids: Vec::new(),
            changed_insight_item_ids: Vec::new(),
            changed_currency_ids: Vec::new(),
            changed_hero_ids: Vec::new(),
            changed_skin_ids: Vec::new(),
            changed_equip_ids: Vec::new(),
            cloth_updates: Vec::new(),
            player_info_changed: false,
        }
    }

    pub(in crate::gm) fn merge_rewards(&mut self, rewards: reward::AppliedRewards) {
        self.player_info_changed |= rewards.player_info_changed;
        self.changed_item_ids
            .extend(rewards.item_ids.into_iter().map(|id| id as i32));
        self.changed_power_item_ids.extend(rewards.power_item_ids);
        self.changed_insight_item_ids
            .extend(rewards.insight_item_ids);
        self.changed_currency_ids
            .extend(rewards.currency_ids.into_iter().map(|(id, _)| id));
        self.changed_hero_ids.extend(rewards.hero_ids);
        self.changed_skin_ids
            .extend(rewards.skin_gains.into_iter().map(|skin| skin.skin_id));
        self.changed_equip_ids.extend(rewards.equip_uids);
        self.cloth_updates.extend(rewards.cloth_updates);
    }

    pub(in crate::gm) fn deduplicate(&mut self) {
        self.changed_item_ids.sort_unstable();
        self.changed_item_ids.dedup();
        self.changed_power_item_ids.sort_unstable();
        self.changed_power_item_ids.dedup();
        self.changed_insight_item_ids.sort_unstable();
        self.changed_insight_item_ids.dedup();
        self.changed_currency_ids.sort_unstable();
        self.changed_currency_ids.dedup();
        self.changed_hero_ids.sort_unstable();
        self.changed_hero_ids.dedup();
        self.changed_skin_ids.sort_unstable();
        self.changed_skin_ids.dedup();
        self.changed_equip_ids.sort_unstable();
        self.changed_equip_ids.dedup();
        let mut cloth_ids = HashSet::new();
        self.cloth_updates.reverse();
        self.cloth_updates
            .retain(|cloth| cloth_ids.insert(cloth.cloth_id));
        self.cloth_updates.reverse();
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct RewardData {
    pub(super) r#type: i32,
    pub(super) id: i32,
    pub(super) amount: i32,
}

pub(in crate::gm) fn parse_positive(value: &str, label: &str) -> Result<i32> {
    let parsed = value.parse::<i32>()?;
    if parsed <= 0 {
        anyhow::bail!("invalid {label} '{value}'");
    }
    Ok(parsed)
}
