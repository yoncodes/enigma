use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        ex_point::ExPointKind,
    },
    skill::{
        buff_act::{BuffActRuleOp, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, transfer_mode, threshold, limit]
        if *buff_id > 0 && *transfer_mode > 0 && *threshold >= 0 && *limit > 0)
}

pub fn referenced_buff(args: &[i32]) -> Option<i32> {
    supports(args).then(|| args[0])
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    let BattleEvent::ExPointOverflow(change) = event else {
        return Some(Vec::new());
    };
    let [buff_id, transfer_mode, reward_per_point, point_limit] = subscriber.args.as_slice() else {
        return None;
    };
    if !super::subscriber_is_kind(subscriber, BuffActKind::TransferEnergyBuff)
        || !subscriber.owner_alive
        || change.kind != ExPointKind::Common
        || *transfer_mode <= 0
        || change.overflow <= 0
        || pool.entity(subscriber.owner_uid).is_none()
        || pool.entity(change.target_uid).is_none()
        || pool.source_is_attacker(subscriber.owner_uid)
            != pool.source_is_attacker(change.target_uid)
        || stores_overflow_moxie(managers, change.target_uid)
    {
        return Some(Vec::new());
    }
    let amount = change
        .overflow
        .min(*point_limit)
        .checked_mul(*reward_per_point)?;
    if amount <= 0 {
        return Some(Vec::new());
    }

    Some(vec![BuffActRuleOp::causing(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: if subscriber.source_uid != 0 {
                subscriber.source_uid
            } else {
                subscriber.owner_uid
            },
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })),
    ))])
}

fn stores_overflow_moxie(managers: &BattleManagers, owner_uid: i64) -> bool {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .any(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(&feature, BuffActKind::ExPointOverflowBank)
        })
}

#[cfg(test)]
mod tests;
