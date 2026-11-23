use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        effect::catalog::{SkillEffectCatalog, SkillEffectTag},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{registry::BuffActKind, subscriber_is_kind};

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !subscriber_is_kind(subscriber, BuffActKind::Frozen) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid
        || catalog.effect_tag(hit.skill_id) != SkillEffectTag::MentalDamage as i32
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Remove(BuffRemove {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }),
    ))])
}

#[cfg(test)]
mod tests;
