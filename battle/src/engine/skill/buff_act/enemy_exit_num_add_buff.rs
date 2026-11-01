use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{BattleManagers, buff::{BuffAccumulateActValue, BuffCommand, BuffGrant}},
    skill::{rule::output::{BattleCommand, RuleOp}, subscriber::BuffActSubscriber, target::TargetPool},
};

pub fn rule_ops(managers: &BattleManagers, pool: &TargetPool, subscriber: &BuffActSubscriber,
    event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::EnemyExitNumAddBuff) { return None; }
    let [buff_id] = subscriber.args.as_slice() else { return None; };
    let origin = super::command_origin(subscriber)?;
    match event {
        BattleEvent::EntityDied(death) => {
            if pool.team_type(death.target_uid).is_none_or(|team| team == subscriber.team_type) { return Some(Vec::new()); }
            Some(vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { origin, target_uid: subscriber.owner_uid, buff_uid: subscriber.buff_uid,
                    act_id: subscriber.key.definition.opcode, delta: 1 },
            )))])
        }
        BattleEvent::RoundStart | BattleEvent::Kind(EventKind::RoundStart) => {
            let count = managers.buff.act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
            if count <= 0 { return Some(Vec::new()); }
            let mut ops = pool.enemies(subscriber.owner_uid, true).iter()
                .filter(|enemy| managers.hp.current(enemy.uid) > 0)
                .map(|enemy| RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    origin, source_uid: subscriber.owner_uid, target_uid: enemy.uid, buff_id: *buff_id,
                    amount: Some(count), occurrences: 1, child_uid_reservations: 0,
                })))).collect::<Vec<_>>();
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { origin, target_uid: subscriber.owner_uid, buff_uid: subscriber.buff_uid,
                    act_id: subscriber.key.definition.opcode, delta: -count },
            ))));
            Some(ops)
        }
        _ => None,
    }
}
