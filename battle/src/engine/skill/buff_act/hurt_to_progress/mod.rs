use crate::engine::{
    event::payload::BattleEvent,
    manager::{fight_progress::FightProgressValueCommand, hp::HurtDamageFromType},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [progress_id, damage_permille] if *progress_id > 0 && *damage_permille > 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [progress_id, damage_permille] = subscriber.args.as_slice() else {
        return None;
    };
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::HurtToProgress)
        || hit.target_uid != subscriber.owner_uid
        || hit.damage_from == HurtDamageFromType::Additional
    {
        return Some(Vec::new());
    }
    let delta = progress_from_damage(hit.resolved_damage, *damage_permille);
    if delta <= 0 {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::FightProgressValue(
        FightProgressValueCommand {
            origin: super::command_origin(subscriber)?,
            progress_id: *progress_id,
            delta,
        },
    ))])
}

fn progress_from_damage(damage: i32, permille: i32) -> i32 {
    ((i64::from(damage.max(0)) * i64::from(permille.max(0))) / 1000).min(i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests;
