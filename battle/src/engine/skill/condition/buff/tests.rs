use super::*;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn parses_static_buff_presence_lists() {
    assert_eq!(
        buff_present(19021, "HasBuffId", &["1,2".to_owned()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![1, 2],
        })
    );
    assert_eq!(
        buff_absent(57304, "NoBuffId", &["1".to_owned()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![1],
        })
    );
    assert_eq!(
        buff_present_and_consume(19208, "HasBuffId", &["8178".to_owned()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::PresentAndConsume,
            buff_ids: vec![8178],
        })
    );
}

#[test]
fn buff_conditions_keep_status_and_added_id_semantics() {
    assert_eq!(
        buff_added(10, "BuffIdAdd", &["31070111".into()]),
        Some(ParsedConditionKind::BuffAdded(vec![31070111]))
    );
    assert_eq!(
        first_status_present(18203, "HasBuff", &["14".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![14],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert_eq!(
        buff_removed(49, "BuffIdDel", &["31070111".into()]),
        Some(ParsedConditionKind::BuffRemoved(vec![31070111]))
    );
}

#[test]
fn skill_action_status_condition_keeps_every_configured_category() {
    assert_eq!(
        any_status_present(18202, "HasBuff", &["2".into(), "4".into(), "6".into()],),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![2, 4, 6],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert_eq!(
        first_status_present(18203, "HasBuff", &["1".into(), "1".into(), "1006".into()],),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![1],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
}

#[test]
fn buff_group_conditions_keep_their_exact_opcode_and_polarity() {
    assert_eq!(
        buff_group(77203, "HasBuffGroup", &["5".into()]),
        Some(ParsedConditionKind::BuffGroup(vec![5]))
    );
    assert_eq!(
        buff_group(77208, "HasBuffGroup", &["5".into()]),
        Some(ParsedConditionKind::BuffGroup(vec![5]))
    );
    assert_eq!(
        no_buff_group(78208, "NoBuffGroup", &["5".into()]),
        Some(ParsedConditionKind::NoBuffGroup(vec![5]))
    );
}

#[test]
fn from_and_to_buff_keeps_both_configured_identities() {
    assert_eq!(
        from_and_to_buff(
            1007204,
            "FromBuffAndToBuff",
            &["229101".into(), "229102".into()],
        ),
        Some(ParsedConditionKind::FromBuffAndToBuff {
            from_buff_id: 229101,
            to_buff_id: 229102,
        })
    );
}

#[test]
fn per_buff_type_layer_returns_the_capped_stack_count() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::default();
    managers.buff.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31340002),
                    layer: Some(7),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 518203,
        type_name: String::new(),
        kind: ParsedConditionKind::PerBuffTypeLayer {
            type_ids: vec![31340002],
            min: 1,
            max: 20,
        },
        raw_args: Vec::new(),
    };

    assert_eq!(
        per_type_layer_count(std::slice::from_ref(&condition), &[10], &managers),
        7
    );
    let below_minimum = ParsedCondition {
        kind: ParsedConditionKind::PerBuffTypeLayer {
            type_ids: vec![31340002],
            min: 8,
            max: 20,
        },
        ..condition
    };
    assert_eq!(per_type_layer_count(&[below_minimum], &[10], &managers), 0);
}
