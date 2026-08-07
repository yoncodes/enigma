use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector, BuffSetState},
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill_id, 1, target_code, initial_count]
        if *skill_id > 0 && *target_code > 0 && *initial_count > 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CountContinueChannel) {
        return None;
    }
    let [skill_id, 1, target_code, _] = subscriber.args.as_slice() else {
        return None;
    };
    let BattleEvent::BuffStateChanged(change) = event else {
        return Some(Vec::new());
    };
    if change.target_uid != subscriber.owner_uid
        || change.buff_uid != subscriber.buff_uid
        || change.buff_id != subscriber.buff_id
        || change.before_ex_info <= 0
        || change.after_ex_info != 0
    {
        return Some(Vec::new());
    }

    let origin = super::command_origin(subscriber)?;
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id: *skill_id,
    });
    invocation.target = SkillTarget::LogicRule(*target_code);
    invocation.mode = SkillExecutionMode::Active;

    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: Some(0),
                params: None,
                act_info: None,
            },
        ))),
        RuleOp::Skill(invocation),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        }))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, payload::BuffStateChangeEvent, subscription::SubscriptionKey},
        skill::rule::DefinitionKey,
    };

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 21,
            buff_id: 31000441,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::BuffStateChanged,
                DefinitionKey::new(838, "CountContinueChannel"),
            ),
            act_type: "CountContinueChannel".to_owned(),
            effect_time: 1041,
            effect_condition: 0,
            args: vec![31000505, 1, 210, 7],
            raw: "838#31000505#1#210#7".to_owned(),
        }
    }

    fn state(after_ex_info: i32) -> BattleEvent {
        BattleEvent::BuffStateChanged(BuffStateChangeEvent {
            source_uid: -1,
            target_uid: -1,
            buff_uid: 21,
            buff_id: 31000441,
            before_ex_info: 3,
            after_ex_info,
        })
    }

    #[test]
    fn completion_snapshots_then_casts_then_removes_the_channel() {
        let ops = rule_ops(&subscriber(), &state(0)).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(_))),
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        skill_id: 31000505,
                        ..
                    },
                    target: SkillTarget::LogicRule(210),
                    ..
                }),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    selector: BuffRemoveSelector::Uid(21),
                    ..
                })))
            ]
        ));
    }

    #[test]
    fn non_terminal_state_does_not_resolve() {
        assert_eq!(rule_ops(&subscriber(), &state(2)), Some(Vec::new()));
    }
}
