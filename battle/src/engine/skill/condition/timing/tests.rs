use super::*;
use crate::engine::skill::condition::{buff::BuffConditionMode, none::NoneMode};

#[test]
fn subscriptions_keep_event_and_exact_opcode() {
    let conditions = [
        ParsedCondition {
            opcode: 210,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::SkillAction),
            raw_args: Vec::new(),
        },
        ParsedCondition {
            opcode: 19004,
            type_name: "HasBuffId".to_owned(),
            kind: ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![30860131],
            },
            raw_args: vec!["30860131".into()],
        },
    ];

    assert_eq!(
        conditions[0].subscriptions(),
        vec![SubscriptionKey::at_phase(
            EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(210, "None"),
            Some(crate::engine::skill::action::SkillPhase::AfterHit),
        )]
    );
    assert_eq!(
        conditions[1].subscriptions(),
        vec![
            SubscriptionKey::new(
                EventKind::BuffAdded,
                crate::engine::skill::rule::DefinitionKey::new(19004, "HasBuffId"),
            ),
            SubscriptionKey::new(
                EventKind::BuffChanged,
                crate::engine::skill::rule::DefinitionKey::new(19004, "HasBuffId"),
            ),
        ]
    );
}

#[test]
fn immediate_power_compare_uses_the_pre_effect_event() {
    let condition = ParsedCondition {
        opcode: 180203,
        type_name: "PowerCompare".to_owned(),
        kind: ParsedConditionKind::PowerCompare {
            compare_code: 2,
            power_id: 1,
            threshold: 1,
        },
        raw_args: Vec::new(),
    };

    assert_eq!(
        condition.timing(),
        ConditionTiming::Event(EventKind::SkillEffectStarted)
    );
    assert_eq!(condition.subscriptions()[0].definition.opcode, 180203);
}

#[test]
fn setup_condition_does_not_become_a_runtime_subscription() {
    let condition = ParsedCondition {
        opcode: 51104,
        type_name: "HasTypeIdBuffMoreThan".to_owned(),
        kind: ParsedConditionKind::BuffTypeCount {
            type_ids: vec![31170006],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        },
        raw_args: Vec::new(),
    };

    assert_eq!(condition.timing(), ConditionTiming::Static);
    assert!(condition.subscriptions().is_empty());
}
