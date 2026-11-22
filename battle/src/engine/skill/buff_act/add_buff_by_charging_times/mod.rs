use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
) -> Vec<RuleOp> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::AddBuffByChargingTimes,
    ) {
        return Vec::new();
    }
    let [buff_id, layer, _mode, divisor] = subscriber.args.as_slice() else {
        return Vec::new();
    };
    let Some(team) = managers.buff.team_type(subscriber.owner_uid) else {
        return Vec::new();
    };
    let progress = managers
        .field
        .get(team)
        .map(|field| field.progress)
        .unwrap_or_default();
    if *buff_id <= 0 || *layer <= 0 || *divisor <= 0 || progress < *divisor {
        return Vec::new();
    }
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let Some(origin) = super::command_origin(subscriber) else {
        return Vec::new();
    };
    pool.main_allies(subscriber.owner_uid)
        .iter()
        .filter(|target| target.current_hp > 0)
        .map(|target| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid,
                target_uid: target.uid,
                buff_id: *buff_id,
                amount: Some(*layer),
                occurrences: 1,
                child_uid_reservations: 0,
            })))
        })
        .collect()
}

#[cfg(test)]
mod tests;
