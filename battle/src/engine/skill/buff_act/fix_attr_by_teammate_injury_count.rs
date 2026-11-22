use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::buff::{ActiveBuffFeature, BuffAccumulateActValue, BuffCommand, BuffManager},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn tracker_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::TeammateInjuryCount,
    ) {
        return Some(Vec::new());
    }
    let delta = action.teammate_injury_count.max(0);
    if delta == 0 {
        return Some(Vec::new());
    }
    let act_id = subscriber.key.definition.opcode;
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::AccumulateActValue(BuffAccumulateActValue {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id,
            delta,
        }),
    ))])
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &BuffManager,
    include_trigger_history: bool,
) -> i32 {
    let [_, raw_attr, amount_per_injury, maximum] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr) != Some(attr_id) {
        return 0;
    }

    (if include_trigger_history {
        buffs.buff_act_value(
            feature.owner_uid,
            super::registry::BuffActKind::TeammateInjuryCount,
        )
    } else {
        feature.amount
    })
    .saturating_mul(*amount_per_injury)
    .min(*maximum)
}

#[cfg(test)]
mod tests;
