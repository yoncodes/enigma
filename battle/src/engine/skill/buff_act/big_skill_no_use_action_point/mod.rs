use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    args.is_empty()
}

pub fn consume_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
        BuffRemove {
            origin: super::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            selector: BuffRemoveSelector::Uid(feature.buff_uid),
        },
    ))))
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::AllyAction(action) = event else {
        return None;
    };
    if action.source_uid != subscriber.owner_uid || !catalog.is_big_skill(action.skill_id) {
        return Some(Vec::new());
    }
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.buff_uid == subscriber.buff_uid)?;
    consume_rule_op(managers, &feature).map(|op| vec![op])
}

#[cfg(test)]
mod tests;
