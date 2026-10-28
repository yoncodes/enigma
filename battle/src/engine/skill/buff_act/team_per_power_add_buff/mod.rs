use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    skill::{
        buff_act::registry::{BuffActKind, ParsedBuffAct},
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [power_id, layers, divisor, buff_id]
        if *power_id > 0 && *layers > 0 && *divisor > 0 && *buff_id > 0)
}

pub fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    RuleReferences {
        buffs: feature
            .args
            .get(3)
            .copied()
            .filter(|buff_id| *buff_id > 0)
            .into_iter()
            .collect(),
        ..RuleReferences::default()
    }
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::TeamPerPowerAddBuff)
        || event.kind() != EventKind::RoundEndEntitySettlement
        || !subscriber.owner_alive
        || !supports(&subscriber.args)
    {
        return Some(Vec::new());
    }
    let [power_id, layers, divisor, buff_id] = subscriber.args.as_slice() else {
        return None;
    };
    let consumed = pool
        .main_allies(subscriber.owner_uid)
        .iter()
        .map(|ally| managers.eureka.round_change(ally.uid, *power_id).consumed)
        .sum::<i32>();
    let amount = consumed / divisor * layers;
    if amount <= 0 {
        return Some(Vec::new());
    }

    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Grant(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

#[cfg(test)]
mod tests;
