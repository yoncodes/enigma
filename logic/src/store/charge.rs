use crate::{
    activity::{Act236ChargeUpdate, act236_charge_update, active_act236_activity_id_at},
    error::AppError,
    reward,
};
use database::db::game::{
    activity236, battle_pass, charges, sign_in, tasks as task_db, user_stats,
};
use sonettobuf::{
    BpPayPush, BpScoreUpdatePush, NewOrderReply, OrderCompletePush, SelectionInfo, StatInfoPush,
};
use sqlx::SqlitePool;

pub struct NewOrderResult {
    pub reply: NewOrderReply,
    pub complete: OrderCompletePush,
    pub stat: StatInfoPush,
    pub bp_pay: Option<BpPayPush>,
    pub bp_score: Option<BpScoreUpdatePush>,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub act236: Option<Act236ChargeUpdate>,
}

pub(super) async fn new_order(
    db: &SqlitePool,
    player_id: i64,
    goods_id: i32,
    currency: Option<String>,
    selections: &[SelectionInfo],
    now: i64,
) -> Result<NewOrderResult, AppError> {
    let tables = config::configs::get();
    let goods = tables
        .store_charge_goods
        .get(goods_id)
        .ok_or(AppError::InvalidRequest)?;

    let mut tx = db.begin().await?;
    let attachment = if let Some(attachment) =
        sign_in::purchase_month_card_attachment_in_transaction(&mut tx, player_id, goods_id, now)
            .await?
    {
        attachment
    } else {
        let mut attachment = charge_goods_attachment(goods);
        for selection in selections {
            let region_id = selection.region_id.unwrap_or_default();
            let selection_pos = selection.selection_pos.unwrap_or_default() as usize;
            let Some(optional) = tables
                .store_charge_optional
                .iter()
                .find(|row| row.goods_id == goods_id && row.id == region_id)
            else {
                continue;
            };
            if let Some(selected) = optional.items.split('|').nth(selection_pos) {
                if !attachment.is_empty() {
                    attachment.push('|');
                }
                attachment.push_str(selected);
            }
        }
        attachment
    };

    let first_purchase =
        charges::is_first_purchase_in_transaction(&mut tx, player_id, goods_id).await?;
    let bp_pushes = apply_battle_pass_purchase(&mut tx, player_id, goods_id).await?;
    let mut parsed_rewards = reward::parse(&attachment);
    parsed_rewards.extend(charge_goods_diamond_bonus(goods, first_purchase));
    parsed_rewards.extend(bp_pushes.rewards);
    let material_changes = parsed_rewards.material_changes();
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, parsed_rewards).await?;
    charges::record_purchase_in_transaction(&mut tx, player_id, goods_id).await?;

    let amount = (goods.price * 100.0) as i64;
    user_stats::update_first_charge_in_transaction(&mut tx, player_id, amount).await?;
    user_stats::set_not_first_login_in_transaction(&mut tx, player_id).await?;
    let stats = user_stats::get_user_stats_in_transaction(&mut tx, player_id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    let act236 = if let Some((activity_id, score_delta)) = active_act236_activity_id_at(now)
        .and_then(|activity_id| {
            tables
                .activity236_charge_score(activity_id, goods_id)
                .map(|score| (activity_id, score))
        }) {
        let state = activity236::increment_score_in_transaction(
            &mut tx,
            player_id,
            activity_id,
            score_delta,
        )
        .await?;
        Some(act236_charge_update(activity_id, state)?)
    } else {
        None
    };
    tx.commit().await?;

    Ok(NewOrderResult {
        reply: NewOrderReply {
            id: Some(goods_id),
            pass_back_param: Some(String::new()),
            notify_url: Some(String::new()),
            game_order_id: Some(now),
            timestamp: Some(now),
            sign: Some("03f92726ce15e0793dddd7f1a9db39f28".to_string()),
            server_id: Some(4),
            currency,
        },
        complete: OrderCompletePush {
            id: Some(goods_id),
            game_order_id: Some(now),
        },
        stat: StatInfoPush {
            frist_charge: Some(stats.first_charge),
            total_charge_amount: Some(stats.total_charge_amount),
            is_first_login: Some(false),
            player_info: None,
            user_tag: Some(stats.user_tag),
        },
        bp_pay: bp_pushes.pay,
        bp_score: bp_pushes.score,
        rewards,
        material_changes,
        act236,
    })
}

pub(crate) fn charge_goods_attachment(
    goods: &config::store_charge_goods::StoreChargeGoods,
) -> String {
    if goods.item.trim().is_empty() {
        goods.product.clone()
    } else {
        goods.item.clone()
    }
}

pub(crate) fn charge_goods_diamond_bonus(
    goods: &config::store_charge_goods::StoreChargeGoods,
    first_purchase: bool,
) -> reward::RewardSet {
    let amount = if first_purchase {
        goods.first_diamond
    } else {
        goods.extra_diamond
    };

    let mut rewards = reward::RewardSet::default();
    if goods.diamond > 0 && amount > 0 {
        rewards.currencies.push((1, amount));
    }
    rewards
}

#[derive(Default)]
struct BattlePassPurchasePushes {
    pay: Option<BpPayPush>,
    score: Option<BpScoreUpdatePush>,
    rewards: reward::RewardSet,
}

async fn apply_battle_pass_purchase(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    player_id: i64,
    goods_id: i32,
) -> Result<BattlePassPurchasePushes, AppError> {
    let Some(bp) = task_db::current_battle_pass() else {
        return Ok(BattlePassPurchasePushes::default());
    };
    let Some((bp_id, pay_status, score_delta)) = battle_pass_pay_status_for(bp, goods_id) else {
        return Ok(BattlePassPurchasePushes::default());
    };

    let update =
        battle_pass::apply_purchase_in_transaction(tx, player_id, bp_id, pay_status, score_delta)
            .await?;
    let rewards = if update.pay_status_changed {
        battle_pass_purchase_bonus_for(bp, update.previous_pay_status, update.pay_status)
    } else {
        reward::RewardSet::default()
    };

    Ok(BattlePassPurchasePushes {
        pay: Some(BpPayPush {
            id: Some(bp_id),
            pay_status: Some(update.pay_status),
        }),
        score: update.score_changed.then_some(BpScoreUpdatePush {
            id: Some(bp_id),
            score: Some(update.score),
            weekly_score: Some(update.weekly_score),
        }),
        rewards,
    })
}

pub(super) fn battle_pass_pay_status_for(
    bp: &config::bp::Bp,
    goods_id: i32,
) -> Option<(i32, i32, i32)> {
    match goods_id {
        id if id == bp.charge_id1 => Some((bp.bp_id, 1, 0)),
        id if id == bp.charge_id2 || id == bp.charge_id1to2 => {
            Some((bp.bp_id, 2, bp.pay_status2_add_level * bp.exp_level_up))
        }
        _ => None,
    }
}

pub(super) fn battle_pass_purchase_bonus_for(
    bp: &config::bp::Bp,
    previous_pay_status: i32,
    pay_status: i32,
) -> reward::RewardSet {
    let mut rewards = reward::RewardSet::default();
    if previous_pay_status < 1 && pay_status >= 1 {
        rewards.extend(reward::parse(&bp.pay_status1_bonus));
    }
    if previous_pay_status < 2 && pay_status >= 2 {
        rewards.extend(reward::parse(&bp.pay_status2_bonus));
    }
    rewards
}
