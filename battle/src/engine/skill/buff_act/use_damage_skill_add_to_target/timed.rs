use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        buff_act::{BuffActRuleOp, command_origin},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub(super) fn apply(
    subscriber: &BuffActSubscriber,
    buff_id: i32,
    targets: Option<&[i64]>,
) -> Option<Vec<BuffActRuleOp>> {
    let Some(targets) = targets else {
        return Some(Vec::new());
    };
    let origin = command_origin(subscriber)?;
    let source_uid = subscriber.owner_uid;
    Some(
        targets
            .iter()
            .copied()
            .filter(|target_uid| *target_uid != 0)
            .flat_map(|target_uid| {
                [
                    RuleOp::BuffFeatureMarker {
                        target_uid: source_uid,
                        effect_type: EffectType::None as i32,
                        effect_num: subscriber.buff_id,
                        buff_act_id: origin.key.opcode,
                    },
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin,
                        source_uid,
                        target_uid,
                        buff_id,
                        amount: None,
                        occurrences: 1,
                        child_uid_reservations: 0,
                    }))),
                ]
                .map(BuffActRuleOp::subscriber_from_applier)
            })
            .collect(),
    )
}
