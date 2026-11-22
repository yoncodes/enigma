use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
        gauge::{GaugeCommand, GaugeOperation},
    },
    mechanic::{bloodtithe, nuo_di_ka::NuoDiKaCommand},
    skill::{
        action::SkillRequest,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [1, cost, max_groups, points_per_group, attack_skill, heal_skill]
        if *cost > 0
            && *max_groups > 0
            && *points_per_group > 0
            && *attack_skill > 0
            && *heal_skill > 0)
}

pub fn referenced_skills(args: &[i32]) -> impl Iterator<Item = i32> + '_ {
    args.get(4..6).into_iter().flatten().copied()
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &crate::engine::skill::effect::SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if event.kind() != EventKind::RoundEndAfterSettlement || !supports(&subscriber.args) {
        return None;
    }
    let [
        _,
        cost,
        max_groups,
        points_per_group,
        attack_skill,
        heal_skill,
    ] = subscriber.args.as_slice()
    else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let key = bloodtithe::rule::key(subscriber.team_type);
    let available = managers.gauge.get(key)?.current.max(0);
    let groups = (available / *cost).min(*max_groups);
    if groups == 0 {
        return Some(vec![RuleOp::Skill(invocation(
            subscriber.owner_uid,
            *heal_skill,
            catalog,
        ))]);
    }

    let consumed = groups * *cost;
    let points = groups * *points_per_group;
    Some(vec![
        RuleOp::Command(BattleCommand::Gauge(
            GaugeCommand::new(
                origin,
                key,
                GaugeOperation::ChangeValue { delta: -consumed },
            )
            .attributed_to(subscriber.owner_uid, 0)
            .with_raw_delta(-consumed.saturating_mul(1000)),
        )),
        RuleOp::Command(BattleCommand::NuoDiKa(NuoDiKaCommand::Set {
            owner_uid: subscriber.owner_uid,
            points,
            bloodtithe_consumed: consumed,
            max_points: max_groups * points_per_group,
        })),
        RuleOp::Skill(invocation(subscriber.owner_uid, *attack_skill, catalog)),
        RuleOp::Command(BattleCommand::NuoDiKa(NuoDiKaCommand::Clear {
            owner_uid: subscriber.owner_uid,
        })),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }))),
    ])
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    catalog: &crate::engine::skill::effect::SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(managers, catalog, subscriber, event).map(|ops| {
        ops.into_iter()
            .enumerate()
            .map(|(index, op)| match index {
                0..=2 => super::BuffActRuleOp::subscriber(op),
                3 => super::BuffActRuleOp::untargeted_event_from_owner(op),
                _ => super::BuffActRuleOp::separate_subscriber_from_owner(op),
            })
            .collect()
    })
}

fn invocation(
    owner_uid: i64,
    skill_id: i32,
    catalog: &crate::engine::skill::effect::SkillEffectCatalog,
) -> crate::engine::skill::action::SkillInvocation {
    let mut invocation: crate::engine::skill::action::SkillInvocation = SkillRequest {
        source_uid: owner_uid,
        skill_id,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    invocation.extra_skill_kind = crate::engine::skill::condition::extra::skill_kind_from_is_extra(
        catalog.extra_kind(skill_id),
    );
    invocation
}

#[cfg(test)]
mod tests;
