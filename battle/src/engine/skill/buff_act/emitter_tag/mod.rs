use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::ActiveBuffFeature},
    mechanic::impromptu,
    skill::{
        buff_act::{BuffActRuleOp, registry::BuffActKind},
        rule::{SetupStage, output::RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn setup_rule_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if stage != SetupStage::BattleStart
        || !super::is_kind(feature, BuffActKind::EmitterTag)
        || !super::is_primary_team_feature(managers, feature, BuffActKind::EmitterTag)
    {
        return Some(Vec::new());
    }
    let enable = impromptu::enable_rule_ops(
        managers.catalog().impromptu_definition(),
        &managers.gauge,
        &managers.buff.active_features(&managers.hp),
        crate::engine::manager::emitter::UID,
    )
    .into_iter()
    .find(|enable| enable.team == feature.team_type)?;
    Some(vec![enable.emitter, enable.team_energy, enable.inspiration])
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::EmitterTag)
        || !super::is_primary_team_subscriber(managers, subscriber, BuffActKind::EmitterTag)
    {
        return Some(Vec::new());
    }
    let ops = match event {
        BattleEvent::ActionQueueCommitted {
            team,
            emitter_uid,
            cards,
            action_count,
        } if *team == subscriber.team_type => impromptu::action_queue_committed_rule_ops(
            managers,
            *team,
            *emitter_uid,
            cards,
            *action_count,
        )
        .into_iter()
        .map(BuffActRuleOp::separate_independent_command)
        .collect(),
        BattleEvent::ImpromptuResolved {
            team, emitter_uid, ..
        } if *team == subscriber.team_type => {
            impromptu::resolved_rule_ops(managers, *team, *emitter_uid)
                .into_iter()
                .map(BuffActRuleOp::causing)
                .collect()
        }
        _ => Vec::new(),
    };
    Some(ops)
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::EurekaChanged(change) = event else {
        return Vec::new();
    };
    if change.applied_delta >= 0 {
        return Vec::new();
    }
    let team = managers.buff.team_type(change.target_uid);
    let Some(feature) = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            Some(feature.team_type) == team && super::is_kind(feature, BuffActKind::EmitterTag)
        })
        .min_by_key(|feature| (feature.owner_uid, feature.buff_uid))
    else {
        return Vec::new();
    };
    impromptu::eureka_spent_rule_op(
        managers,
        change.target_uid,
        change.applied_delta.saturating_abs(),
    )
    .map(|op| vec![(feature, op)])
    .unwrap_or_default()
}

#[cfg(test)]
mod tests;
