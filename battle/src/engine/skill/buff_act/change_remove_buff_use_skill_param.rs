use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::{feature_command_origin, is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [minimum_delta, maximum_delta]
        if minimum_delta == maximum_delta && *minimum_delta != 0)
}

pub fn adjust_range(
    managers: &BattleManagers,
    owner_uid: i64,
    minimum: i32,
    maximum: i32,
) -> Option<(i32, i32, Vec<RuleOp>)> {
    let mut minimum = minimum;
    let mut maximum = maximum;
    let mut consumes = Vec::new();
    for feature in managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && feature.amount > 0
                && is_kind(feature, BuffActKind::ChangeRemoveBuffUseSkillParam)
                && supports(feature.values.get(1..).unwrap_or_default())
        })
    {
        minimum = minimum.checked_add(feature.values[1])?;
        maximum = maximum.checked_add(feature.values[2])?;
        consumes.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                origin: feature_command_origin(&feature)?,
                target_uid: owner_uid,
                selector: BuffSelector::Uid(feature.buff_uid),
                amount: 1,
                depleted: DepletedBuff::Remove,
            },
        ))));
    }
    (minimum > 0 && maximum >= minimum).then_some((minimum, maximum, consumes))
}
