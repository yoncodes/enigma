use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::BuffStatus, revive::ReviveCommand},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [0 | 1, raw_attr, permille]
        if AttrId::from_raw(*raw_attr).is_some() && *permille > 0)
}

pub fn supports_dying_heal(args: &[i32]) -> bool {
    matches!(args, [permille, statuses @ ..]
        if *permille > 0
            && !statuses.is_empty()
            && statuses.iter().all(|status| BuffStatus::from_id(*status).is_bad()))
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::EntityDied(death) = event else {
        return None;
    };
    if death.target_uid != subscriber.owner_uid || subscriber.owner_alive {
        return Some(Vec::new());
    }
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let (amount, dispel_statuses) = if matches!(
        super::subscriber_kind(subscriber),
        Some(super::registry::BuffActKind::Revive | super::registry::BuffActKind::Cure)
    ) {
        let [mode, raw_attr, permille] = subscriber.args.as_slice() else {
            return None;
        };
        if !supports(&subscriber.args) {
            return None;
        }
        let basis_uid = if *mode == 0 {
            source_uid
        } else {
            subscriber.owner_uid
        };
        (
            managers
                .origin_attribute(basis_uid, AttrId::from_raw(*raw_attr)?)
                .max(0)
                .saturating_mul(*permille)
                / 1000,
            Vec::new(),
        )
    } else if super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::DyingHealDisperse1,
    ) {
        let [permille, statuses @ ..] = subscriber.args.as_slice() else {
            return None;
        };
        if !supports_dying_heal(&subscriber.args) {
            return None;
        }
        (
            managers.hp.max(subscriber.owner_uid).max(0) * *permille / 1000,
            statuses.iter().copied().map(BuffStatus::from_id).collect(),
        )
    } else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    Some(if amount > 0 {
        vec![RuleOp::Command(BattleCommand::Revive(ReviveCommand {
            origin,
            source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            buff_uid: subscriber.buff_uid,
            dispel_statuses,
        }))]
    } else {
        Vec::new()
    })
}

#[cfg(test)]
mod tests;
