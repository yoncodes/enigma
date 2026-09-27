use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        action::{SkillInvocation, SkillRequest},
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CastChannel)
        || event.kind() != EventKind::RoundStart
    {
        return None;
    }
    let skill_id = referenced_skill(&subscriber.args)?;
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::Skill(SkillInvocation::from(SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id,
        })),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }))),
    ])
}

pub fn scoped_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(subscriber, event).map(|ops| {
        ops.into_iter()
            .enumerate()
            .map(|(index, op)| match index {
                0 => super::BuffActRuleOp::subscriber(op),
                _ => super::BuffActRuleOp::separate_subscriber_from_owner(op),
            })
            .collect()
    })
}

pub fn referenced_skill(args: &[i32]) -> Option<i32> {
    let [skill_id, _, _, _] = args else {
        return None;
    };
    (*skill_id > 0).then_some(*skill_id)
}

pub fn supports(args: &[i32]) -> bool {
    referenced_skill(args).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey};

    #[test]
    fn round_start_casts_then_removes_the_channel() {
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStart,
                DefinitionKey::new(731, "CastChannel"),
            ),
            act_type: "CastChannel".to_owned(),
            effect_time: 1041,
            effect_condition: 0,
            args: vec![40, 1, 1, 1],
            raw: "731#40#1#1#1".to_owned(),
        };

        assert!(matches!(
            rule_ops(&subscriber, &BattleEvent::RoundStart).as_deref(),
            Some([
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        source_uid: 10,
                        skill_id: 40,
                    },
                    ..
                }),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    target_uid: 10,
                    selector: BuffRemoveSelector::Uid(20),
                    ..
                })))
            ])
        ));
    }
}
