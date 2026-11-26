use crate::engine::{
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
    },
    skill::rule::output::{BattleCommand, RuleOp},
};

pub(in crate::engine::skill::buff_act) fn consume(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
) -> Option<RuleOp> {
    if !matches!(feature.values.as_slice(), [_, _, _, 1, 1]) {
        return super::consume_rule_op(managers, feature);
    }
    managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    Some(RuleOp::Command(BattleCommand::Buff(
        BuffCommand::RemoveAfterTrigger(BuffRemove {
            origin: super::super::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            selector: BuffRemoveSelector::Uid(feature.buff_uid),
        }),
    )))
}
