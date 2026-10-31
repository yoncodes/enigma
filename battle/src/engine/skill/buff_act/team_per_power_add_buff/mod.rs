use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand, BuffGrant},
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
    let previous = managers
        .buff
        .act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
    let total = previous.saturating_add(consumed);
    let amount = total / divisor * layers;
    let remainder = total % divisor;
    let mut ops = Vec::with_capacity(2);
    let origin = super::command_origin(subscriber)?;
    if amount > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                buff_id: *buff_id,
                amount: Some(amount),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    let delta = remainder - previous;
    if delta != 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta,
            }),
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
