use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand, BuffGrant},
    },
    skill::{
        buff_act::registry::ParsedBuffAct,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [threshold, reward, layers] if *threshold > 0 && *reward > 0 && *layers > 0)
}

pub fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    RuleReferences {
        buffs: feature
            .args
            .get(1)
            .copied()
            .filter(|id| *id > 0)
            .into_iter()
            .collect(),
        ..RuleReferences::default()
    }
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::FieldTransferred { team, amount } = event else {
        return None;
    };
    let &[threshold, buff_id, layers] = subscriber.args.as_slice() else {
        return None;
    };
    if !subscriber.owner_alive || *team != subscriber.team_type || *amount <= 0 {
        return Some(Vec::new());
    }
    let act_id = subscriber.key.definition.opcode;
    let previous = managers.buff.act_value(subscriber.buff_uid, act_id);
    let total = i64::from(previous) + i64::from(*amount);
    let remainder = (total % i64::from(threshold)) as i32;
    let grant = (total / i64::from(threshold) * i64::from(layers)).min(i64::from(i32::MAX)) as i32;
    let origin = super::command_origin(subscriber)?;

    let mut ops = Vec::with_capacity(2);
    if remainder != previous {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id,
                delta: remainder - previous,
            }),
        )));
    }
    if grant > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                buff_id,
                amount: Some(grant),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
