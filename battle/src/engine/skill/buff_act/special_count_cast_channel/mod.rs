use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        buff_act::registry::BuffActKind,
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::SpecialCountCastChannel) {
        return None;
    }
    if !matches!(
        event,
        BattleEvent::Kind(EventKind::RoundEndEntitySettlement)
    ) {
        return Some(Vec::new());
    }
    let skill_id = subscriber.args.first().copied().filter(|id| *id > 0)?;
    let origin = super::command_origin(subscriber)?;
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(skill_id));
    Some(vec![
        RuleOp::Skill(invocation),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }))),
    ])
}

pub fn scoped_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(subscriber, event, catalog).map(|ops| {
        ops.into_iter()
            .enumerate()
            .map(|(index, op)| match index {
                0 => super::BuffActRuleOp::subscriber(op),
                _ => super::BuffActRuleOp::separate_subscriber_from_owner(op),
            })
            .collect()
    })
}

#[cfg(test)]
mod tests;
