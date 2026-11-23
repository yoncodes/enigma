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
