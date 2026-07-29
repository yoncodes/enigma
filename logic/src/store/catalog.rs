use super::{goods_store_id, is_time_active, parse_time_millis};
use crate::error::AppError;
use common::time::ServerTime;
use database::db::game::{charges, store};
use sonettobuf::{ChargeInfo, GetStoreInfosReply, GoodsInfo, StoreInfo};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub(super) async fn store_infos(
    db: &SqlitePool,
    player_id: i64,
    requested_store_ids: &[i32],
) -> Result<GetStoreInfosReply, AppError> {
    let tables = config::configs::get();
    let store_ids = if requested_store_ids.is_empty() {
        tables.store.iter().map(|store| store.id).collect()
    } else {
        requested_store_ids.to_vec()
    };
    let buy_counts = store::get_buy_counts(db, player_id).await?;

    let store_infos = store_ids
        .into_iter()
        .filter_map(|store_id| {
            let configured_goods = tables
                .store_goods
                .iter()
                .filter(|goods| goods_store_id(&goods.store_id) == Some(store_id))
                .collect::<Vec<_>>();
            let goods_infos = configured_goods
                .iter()
                .filter(|goods| {
                    goods.is_online
                        && is_time_active(
                            &goods.online_time,
                            &goods.offline_time,
                            ServerTime::now_ms(),
                        )
                })
                .map(|goods| {
                    let offline_time = parse_time_millis(&goods.offline_time);
                    GoodsInfo {
                        goods_id: goods.id,
                        buy_count: buy_counts.get(&goods.id).copied().unwrap_or_default(),
                        offline_time: (offline_time > 0).then_some(offline_time),
                    }
                })
                .collect::<Vec<_>>();

            (!goods_infos.is_empty()
                || (configured_goods.is_empty() && tables.store.get(store_id).is_some()))
            .then_some(StoreInfo {
                id: store_id,
                next_refresh_time: 0,
                goods_infos,
                offline_time: None,
            })
        })
        .collect();

    Ok(GetStoreInfosReply { store_infos })
}

pub(super) async fn charge_infos(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Vec<ChargeInfo>, AppError> {
    let purchases = charges::get_charge_infos(db, player_id)
        .await?
        .into_iter()
        .map(|info| (info.charge_id, info))
        .collect::<HashMap<_, _>>();
    let now = ServerTime::now_ms();

    Ok(config::configs::get()
        .store_charge_goods
        .iter()
        .filter(|goods| {
            goods.is_online && is_time_active(&goods.online_time, &goods.offline_time, now)
        })
        .map(|goods| {
            let purchase = purchases.get(&goods.id);

            ChargeInfo {
                id: Some(goods.id),
                buy_count: Some(purchase.map_or(0, |info| info.buy_count)),
                first_charge: Some(
                    goods.first_diamond > 0 && purchase.is_none_or(|info| info.first_charge),
                ),
            }
        })
        .collect())
}
