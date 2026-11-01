use crate::engine::{
    event::kind::EventKind,
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{rule::output::RuleOp, subscriber::BuffActSubscriber, target::TargetPool},
};

pub fn rule_ops(managers: &BattleManagers, pool: &TargetPool, random: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::ExtraPoisonSettlement)
        || subscriber.args != [1, 1] { return None; }
    let features = managers.buff.active_features_for_owner(&managers.hp, subscriber.owner_uid);
    let mut ops = Vec::new();
    for feature in features.into_iter().filter(|f| super::is_kind(f, super::registry::BuffActKind::Poison)
        || super::is_kind(f, super::registry::BuffActKind::PowerfulPoison)) {
        let Some(poison) = super::subscriber_from_feature(feature, EventKind::RoundEnd) else { continue; };
        for _ in 0..subscriber.amount.max(1) {
            if super::subscriber_is_kind(&poison, super::registry::BuffActKind::PowerfulPoison) {
                ops.extend(super::powerful_poison::settlement_ops(managers, pool, random, &poison, false).unwrap_or_default());
            } else if let Some(op) = super::damage_over_time::detonation_rule_op(managers, pool, random, &poison, false) {
                ops.push(op);
            }
        }
    }
    Some(ops)
}
