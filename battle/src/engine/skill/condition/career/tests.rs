use super::*;

#[test]
fn semantic_parsers_construct_their_owned_kind() {
    assert!(matches!(
        target_career(16002, "TargetCareer", &["3".into()]),
        Some(ParsedConditionKind::TargetCareer(_))
    ));
    assert!(matches!(
        parse_career_check(508212, "CareerCheck", &[]),
        Some(ParsedConditionKind::TargetSharesCasterCareer { param: 0 })
    ));
}

#[test]
fn battle_rule_career_filter_keeps_all_configured_afflatuses() {
    assert_eq!(
        target_career(
            16204,
            "TargetCareer",
            &["1".into(), "2".into(), "3".into(), "4".into()],
        ),
        Some(ParsedConditionKind::TargetCareer(vec![1, 2, 3, 4]))
    );
}

#[test]
fn natural_ally_count_keeps_the_configured_cap() {
    assert_eq!(
        natural_ally_count(621002, "CareerNatureHeroNum", &["3".into()]),
        Some(ParsedConditionKind::PerTargetCareerCount {
            careers: vec![1, 2, 3, 4],
            threshold: 3,
        })
    );
}

#[test]
fn per_team_career_count_keeps_the_configured_side() {
    assert_eq!(
        per_team_career_count(
            517010,
            "PerHasTargetCareer",
            &["2".into(), "1".into(), "1".into()],
        ),
        Some(ParsedConditionKind::PerTeamCareerCount {
            career: 2,
            divisor: 1,
            opposing_team: false,
        })
    );
    assert_eq!(
        per_team_career_count(
            517010,
            "PerHasTargetCareer",
            &["3".into(), "1".into(), "2".into()],
        ),
        Some(ParsedConditionKind::PerTeamCareerCount {
            career: 3,
            divisor: 1,
            opposing_team: true,
        })
    );
}

#[test]
fn team_career_threshold_keeps_the_configured_group_and_minimum() {
    assert_eq!(
        team_career_count_at_least(562002, "CareerGroupHeroCountGE", &["3".into(), "3".into()],),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![3],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert_eq!(
        team_career_count_at_most(
            560100,
            "CareerGroupHeroCountLE",
            &["3,5,6".into(), "2".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![3, 5, 6],
            compare: ConditionCompare::LessThanOrEqual,
            threshold: 2,
        })
    );
}

#[test]
fn target_team_career_count_keeps_the_exact_four_field_contract() {
    assert_eq!(
        target_team_career_count(
            516101,
            "HasTargetCareerNum",
            &["4".into(), "3".into(), "2".into(), "1".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![4],
            compare: ConditionCompare::Equal,
            threshold: 2,
        })
    );
    assert_eq!(
        target_team_career_count(
            516101,
            "HasTargetCareerNum",
            &["4".into(), "1".into(), "3".into(), "1".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![4],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
}

#[test]
fn target_team_career_count_rejects_unobserved_shapes() {
    let parse = |args: &[&str]| {
        target_team_career_count(
            516101,
            "HasTargetCareerNum",
            &args
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>(),
        )
    };

    assert_eq!(parse(&["4", "1", "3"]), None);
    assert_eq!(parse(&["4", "1", "3", "1", "0"]), None);
    assert_eq!(parse(&["4", "2", "3", "1"]), None);
    assert_eq!(parse(&["4", "1", "3", "2"]), None);
    assert_eq!(parse(&["0", "1", "3", "1"]), None);
    assert_eq!(parse(&["4", "1", "0", "1"]), None);
}
