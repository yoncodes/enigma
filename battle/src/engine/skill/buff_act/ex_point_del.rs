use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::ex_point::{ExPointChange, ExPointCommand},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [amount] if *amount > 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber) -> Option<Vec<RuleOp>> {
    let [amount] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::Change(ExPointChange {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            delta: -amount,
            config_effect: 0,
            effect_type: EffectType::Expointchange as i32,
        }),
    ))])
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::rule::DefinitionKey,
    };

    use super::*;

    #[test]
    fn round_end_loss_targets_the_buff_holder() {
        let subscriber = BuffActSubscriber {
            owner_uid: 20,
            source_uid: -3,
            buff_uid: 7,
            buff_id: 22301872,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(EventKind::RoundEnd, DefinitionKey::new(605, "ExPointDel")),
            act_type: "ExPointDel".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![1],
            raw: "605#1".to_owned(),
        };

        assert!(matches!(
            rule_ops(&subscriber).as_deref(),
            Some([RuleOp::Command(BattleCommand::ExPoint(
                ExPointCommand::Change(ExPointChange {
                    source_uid: -3,
                    target_uid: 20,
                    delta: -1,
                    ..
                })
            ))])
        ));
    }
}
