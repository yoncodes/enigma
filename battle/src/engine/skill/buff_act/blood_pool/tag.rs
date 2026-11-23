use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::ActiveBuffFeature},
    mechanic::{bloodtithe, heat_scale},
    skill::{
        buff_act::{self, BuffActRuleOp, registry::BuffActKind},
        effect::SkillEffectCatalog,
        rule::{SetupStage, output::RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn setup_rule_ops(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if !buff_act::is_kind(feature, BuffActKind::BloodPoolTag)
        || !buff_act::is_primary_team_feature(managers, feature, BuffActKind::BloodPoolTag)
    {
        return Some(Vec::new());
    }
    let features = managers.buff.active_features(&managers.hp);
    match stage {
        SetupStage::BattleStart => {
            if heat_scale::creation_specs(&features, catalog)
                .into_iter()
                .any(|create| create.team == feature.team_type)
            {
                return Some(Vec::new());
            }
            Some(
                bloodtithe::rule::enable_rule_op(managers, pool, feature)
                    .into_iter()
                    .collect(),
            )
        }
        SetupStage::Unconditional | SetupStage::RoundStart => Some(
            bloodtithe::rule::sync_base_max_rule_op(managers, feature)
                .into_iter()
                .collect(),
        ),
        _ => Some(Vec::new()),
    }
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !buff_act::subscriber_is_kind(subscriber, BuffActKind::BloodPoolTag) {
        return Some(Vec::new());
    }
    let primary =
        buff_act::is_primary_team_subscriber(managers, subscriber, BuffActKind::BloodPoolTag);
    if !primary {
        return Some(Vec::new());
    }
    let ops = match event {
        BattleEvent::HpLost {
            origin,
            skill_id,
            target_uid,
            amount,
            buff_uid,
            ..
        } if managers.buff.team_type(*target_uid) == Some(subscriber.team_type) => {
            let observed = buff_act::lost_hp_add_extra_blood_pool_value::observed_loss(
                managers,
                *target_uid,
                *buff_uid,
                *amount,
            );
            if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Gauge) {
                eprintln!(
                    "bloodtithe hp-loss team={} source={} skill={} target={} actual={} observed={} buff_uid={buff_uid:?}",
                    subscriber.team_type,
                    subscriber.source_uid,
                    skill_id,
                    target_uid,
                    amount,
                    observed,
                );
            }
            bloodtithe::rule::hp_loss_event_rule_op(
                managers,
                *origin,
                subscriber.team_type,
                *target_uid,
                observed,
                *skill_id,
            )
        }
        BattleEvent::GaugeChanged(change) => {
            bloodtithe::rule::faith_ex_point_rule_ops(managers, change)
        }
        _ => Vec::new(),
    };
    Some(ops.into_iter().map(BuffActRuleOp::causing).collect())
}
