use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrantChild},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(managers: &BattleManagers, subscriber: &BuffActSubscriber) -> Vec<RuleOp> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::SpecialCountContinueChannelBuff,
    ) {
        return Vec::new();
    }

    let outputs = managers
        .buff
        .special_count_outputs(&managers.hp)
        .into_iter()
        .filter(|output| {
            output.source_uid == subscriber.owner_uid && output.marker_buff_id == subscriber.buff_id
        })
        .collect::<Vec<_>>();
    if outputs.is_empty() {
        return Vec::new();
    }

    let mut ops = vec![RuleOp::BuffFeatureMarker {
        target_uid: subscriber.owner_uid,
        effect_type: sonettobuf::effect_type_enum::EffectType::Triggeranalysis as i32,
        effect_num: 0,
        buff_act_id: 0,
    }];
    for output in outputs {
        let Some(origin) = managers.catalog().buff_act_origin(
            output.output_act_id,
            super::registry::BuffActKind::AddAttrBySpecialCount,
        ) else {
            continue;
        };
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::GrantChild(BuffGrantChild {
                origin,
                source_uid: output.source_uid,
                target_uid: output.target_uid,
                buff_id: output.output_buff_id,
                amount: Some(0),
                params: Some(format!("{}#{}", output.output_act_id, output.amount)),
                act_info: None,
            }),
        )));
        ops.push(RuleOp::BuffFeatureMarker {
            target_uid: output.target_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::Attr as i32,
            effect_num: 0,
            buff_act_id: 0,
        });
    }
    ops
}

#[cfg(test)]
mod tests;
