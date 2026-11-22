use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        emanation::EmanationKind,
    },
    skill::{
        action::{SkillExecutionMode, SkillPhase},
        condition::extra::skill_kind_from_is_extra,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    feature: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if action.phase != SkillPhase::AfterDamage
        || feature.key.event != crate::engine::event::kind::EventKind::SkillAction
        || !action.is_attack
    {
        return Some(Vec::new());
    }
    let [
        buff_id,
        blue_layer,
        purple_layer,
        green_rank_two,
        green_rank_three,
        ..,
    ] = feature.args.as_slice()
    else {
        return None;
    };
    if *buff_id <= 0 {
        return Some(Vec::new());
    }
    let is_extra_action = skill_kind_from_is_extra(action.extra_skill_kind)
        .is_some_and(|kind| kind.is_extra_action());
    let blue = if is_extra_action { *blue_layer } else { 0 };
    let purple = if !is_extra_action
        && matches!(
            action.mode,
            SkillExecutionMode::Active | SkillExecutionMode::DirectBig
        ) {
        *purple_layer
    } else {
        0
    };
    let layer = managers
        .emanation
        .count(feature.source_uid, EmanationKind::Blue)
        * blue
        + managers
            .emanation
            .count(feature.source_uid, EmanationKind::Purple)
            * purple
        + managers
            .emanation
            .count(feature.source_uid, EmanationKind::Green)
            * match action.rank {
                2 => *green_rank_two,
                3 => *green_rank_three,
                _ => 0,
            };
    if layer <= 0 {
        return Some(Vec::new());
    }

    let source_uid = action.source_uid;
    let origin = super::command_origin(feature).expect("registered crystal buff act");
    let commands = action
        .target_uids
        .iter()
        .copied()
        .filter(|target_uid| *target_uid != 0)
        .map(|target_uid| {
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid,
                target_uid,
                buff_id: *buff_id,
                amount: Some(layer),
                occurrences: 1,
                child_uid_reservations: 0,
            })
        })
        .collect::<Vec<_>>();
    Some(
        commands
            .into_iter()
            .map(|command| RuleOp::Command(BattleCommand::Buff(command)))
            .collect(),
    )
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    feature: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(managers, feature, event).map(|ops| {
        ops.into_iter()
            .map(super::BuffActRuleOp::subscriber_from_applier)
            .collect()
    })
}

#[cfg(test)]
mod tests;
