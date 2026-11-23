use super::*;

#[test]
fn target_include_hero_595002_is_an_enter_fight_gate() {
    assert!(matches!(
        target_model(595002, "TargetIncludeHero", &["3091".into()]),
        Some(ParsedConditionKind::TargetIdentity {
            mode: TargetIdentityMode::TargetModelId,
            value: 3091,
        })
    ));
}

#[test]
fn ex_skill_level_751210_parses_strict_rank_sets() {
    assert_eq!(
        ex_skill_levels(751210, "ExSkillLevel", &["0,1,2,3,4".into()]),
        Some(ParsedConditionKind::ExSkillLevels(vec![0, 1, 2, 3, 4]))
    );
    assert_eq!(
        ex_skill_levels(751210, "ExSkillLevel", &["5".into()]),
        Some(ParsedConditionKind::ExSkillLevels(vec![5]))
    );
    for args in [
        vec![],
        vec!["".into()],
        vec!["0,,1".into()],
        vec!["-1".into()],
        vec!["6".into()],
        vec!["0,5".into()],
        vec!["4,3,2,1,0".into()],
        vec!["0,1,2,3,4,4".into()],
        vec![" 5 ".into()],
        vec!["0，1，2，3，4".into()],
        vec!["0,1".into(), "2".into()],
    ] {
        assert!(ex_skill_levels(751210, "ExSkillLevel", &args).is_none());
    }
}

#[test]
fn target_include_hero_595101_requires_one_positive_model() {
    assert!(matches!(
        positive_target_model(595101, "TargetIncludeHero", &["3102".into()]),
        Some(ParsedConditionKind::TargetIdentity {
            mode: TargetIdentityMode::TargetModelId,
            value: 3102,
        })
    ));
    assert!(positive_target_model(595101, "TargetIncludeHero", &[]).is_none());
    assert!(
        positive_target_model(595101, "TargetIncludeHero", &["3102".into(), "3121".into()])
            .is_none()
    );
    assert!(positive_target_model(595101, "TargetIncludeHero", &["0".into()]).is_none());
}

#[test]
fn target_include_hero_595210_requires_one_positive_active_source_model() {
    assert!(matches!(
        positive_active_skill_source_model(595210, "TargetIncludeHero", &["3102".into()]),
        Some(ParsedConditionKind::TargetIdentity {
            mode: TargetIdentityMode::ActiveSkillSourceModelId,
            value: 3102,
        })
    ));
    assert!(positive_active_skill_source_model(595210, "TargetIncludeHero", &[]).is_none());
    assert!(
        positive_active_skill_source_model(
            595210,
            "TargetIncludeHero",
            &["3102".into(), "3121".into()]
        )
        .is_none()
    );
    assert!(
        positive_active_skill_source_model(595210, "TargetIncludeHero", &["0".into()]).is_none()
    );
}

#[test]
fn ezio_roster_gate_requires_the_listed_hero_on_the_team() {
    assert_eq!(
        team_model_presence(
            643004,
            "HasConditionTarget",
            &["6".into(), "3122,3124".into(), "0".into(), "1".into()]
        ),
        Some(ParsedConditionKind::TeamModelPresence {
            model_ids: vec![3122, 3124],
            present: true,
        })
    );
    assert!(
        team_model_presence(
            643004,
            "HasConditionTarget",
            &["3".into(), "5".into(), "0".into(), "1".into()]
        )
        .is_none()
    );
}

#[test]
fn team_contains_hero_preserves_every_configured_model() {
    assert_eq!(
        team_contains_model(1000212, "TeamContainHero", &["3122,3123".into()]),
        Some(ParsedConditionKind::TeamContainsModels(vec![3122, 3123]))
    );
}
