use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::{feature_command_origin, is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [reduction] if *reduction > 0)
}

pub fn modifier(
    managers: &BattleManagers,
    source_uid: i64,
    skill_id: i32,
) -> Option<(i32, RuleOp)> {
    let (team, skill) = managers.conduit.configured_skill(source_uid, skill_id)?;
    if skill.cost_type == 999 || skill.cost_value <= 0 {
        return None;
    }
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == source_uid
                && feature.team_type == team
                && feature.amount > 0
                && is_kind(feature, BuffActKind::DeviceCostReduce)
                && supports(feature.values.get(1..).unwrap_or_default())
        })?;
    let reduction = feature.values[1].min(skill.cost_value);
    let consume = RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
        origin: feature_command_origin(&feature)?,
        target_uid: feature.owner_uid,
        selector: BuffSelector::Uid(feature.buff_uid),
        amount: 1,
        depleted: DepletedBuff::Remove,
    })));
    Some((reduction, consume))
}

#[cfg(test)]
mod tests;
