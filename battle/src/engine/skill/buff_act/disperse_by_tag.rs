use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [amount, tags @ ..] if *amount > 0 && !tags.is_empty())
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let (&amount, tags) = subscriber.args.split_first()?;
    if !action.target_uids.contains(&subscriber.owner_uid) || !tags.contains(&action.effect_tag) {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Consume(BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

#[cfg(test)]
mod tests;
