use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        injury::{InjuryCommand, InjuryRecord},
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
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::HpLost {
        source_uid,
        target_uid,
        ..
    } = event
    else {
        return None;
    };
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::ControlTeamInjuryCountRound,
    ) || !super::is_primary_team_subscriber(
        managers,
        subscriber,
        super::registry::BuffActKind::ControlTeamInjuryCountRound,
    ) || pool.team_type(*target_uid) != Some(subscriber.team_type)
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Injury(
        InjuryCommand::RecordAction(InjuryRecord {
            origin: super::command_origin(subscriber)?,
            source_uid: *source_uid,
            team_type: subscriber.team_type,
            injured_targets: vec![*target_uid],
            counter_owner_uid: subscriber.owner_uid,
        }),
    ))])
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(managers, pool, subscriber, event)
        .map(|ops| ops.into_iter().map(super::BuffActRuleOp::event).collect())
}

#[cfg(test)]
mod tests;
