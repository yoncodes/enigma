use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        gauge::{GaugeChangeKind, GaugeKind, GaugeOwner},
    },
    skill::{
        action::{SkillRequest, SkillStart},
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp, ThresholdSkillCommand},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [_, threshold, skill_id] if *threshold > 0 && *skill_id > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    _catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::GaugeChanged(change) = event else {
        return None;
    };
    let GaugeOwner::Team(team) = change.key.owner else {
        return Some(Vec::new());
    };
    let progress_delta = change.progress_raw_delta;
    if change.key.kind != GaugeKind::Bloodtithe
        || change.kind != GaugeChangeKind::Value
        || team != subscriber.team_type
        || progress_delta <= 0
    {
        return Some(Vec::new());
    }
    let [required_buff_id, threshold, skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    if !managers
        .buff
        .has_buff_id_or_type(subscriber.owner_uid, *required_buff_id)
    {
        return Some(Vec::new());
    }
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Gauge) {
        eprintln!(
            "bloodtithe threshold source_skill={} owner={} delta={} threshold={} emitted_skill={}",
            change.source_skill_id,
            subscriber.owner_uid,
            progress_delta,
            threshold.saturating_mul(1000),
            skill_id,
        );
    }
    let mut invocation: crate::engine::skill::action::SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id: *skill_id,
    }
    .into();
    invocation.start = if crate::engine::skill::buff_act::raspberry::owns_loss(change.origin) {
        SkillStart::AfterLossBatch
    } else {
        SkillStart::AfterCurrentAction
    };
    Some(vec![RuleOp::Command(BattleCommand::ThresholdSkill(
        ThresholdSkillCommand {
            owner_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            key: subscriber.key.definition,
            threshold: threshold.saturating_mul(1000),
            delta: progress_delta,
            invocation,
        },
    ))])
}

#[cfg(test)]
mod tests;
