use super::*;

#[test]
fn bendith_rules_keep_distinct_exact_routes_and_structured_support() {
    crate::test_support::init_config();
    let replacement = find(1138, "ReplaceEntitySkillGroup").unwrap();
    assert_eq!(replacement.kind, BuffActKind::ReplaceEntitySkillGroup);
    assert_eq!(
        replacement.transaction.events,
        &[EventKind::BuffAdded, EventKind::BuffRemoved]
    );
    assert_eq!(
        destination_with_raw(
            config::try_get(),
            1138,
            "ReplaceEntitySkillGroup",
            &[],
            Some("1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223")
        ),
        Some(BuffActDestination::Transaction)
    );
    assert_eq!(
        destination_with_raw(
            config::try_get(),
            1138,
            "ReplaceEntitySkillGroup",
            &[],
            Some("1138#1:31460211#2:31460221")
        ),
        None
    );

    let no_cost = find(1140, "SkillNoUseActPoint").unwrap();
    assert_eq!(no_cost.kind, BuffActKind::SkillNoUseActPoint);
    assert_eq!(
        destination(1140, "SkillNoUseActPoint", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(destination(1140, "SkillNoUseActPoint", &[1]), None);
    assert!(find(1140, "BigSkillNoUseActPoint").is_none());
}

#[test]
fn generic_feature_arguments_never_compact_invalid_cells() {
    crate::test_support::init_config();

    let invalid = resolve_feature(config::try_get(), "879#1#bad#2").unwrap();
    assert!(invalid.values.is_empty());
    assert_eq!(
        invalid.parse_error,
        Some(BuffActParseError::InvalidInteger { cell: 2, item: 0 })
    );

    let empty = resolve_feature(config::try_get(), "879#1##2").unwrap();
    assert!(empty.values.is_empty());
    assert_eq!(
        empty.parse_error,
        Some(BuffActParseError::EmptyArgument { cell: 2, item: 0 })
    );

    let valid = resolve_feature(config::try_get(), "879#1#300#1,2").unwrap();
    assert_eq!(valid.values, [879, 1, 300, 1, 2]);
    assert!(valid.parse_error.is_none());
}

#[test]
fn structured_feature_grammars_are_exact_registry_metadata() {
    crate::test_support::init_config();
    let game = config::configs::get();
    let mut structured = 0;

    for buff in game.skill_buff.iter() {
        for raw in buff
            .features
            .split('|')
            .map(str::trim)
            .filter(|raw| !raw.is_empty())
        {
            let Some(feature) = resolve_feature(Some(game), raw) else {
                continue;
            };
            let Some(definition) = feature.definition else {
                continue;
            };
            let cells = raw.split('#').map(str::to_owned).collect::<Vec<_>>();
            if parse_integer_projection(&cells).is_err() {
                structured += 1;
                assert!(
                    definition.parser.is_some(),
                    "registered structured feature lacks an exact parser: {raw}"
                );
                assert!(
                    feature.parse_error.is_none(),
                    "exact structured parser rejected configured feature: {raw}"
                );
            }
        }
    }

    assert!(structured > 0);
}

#[test]
fn structured_features_keep_semantic_values_and_references() {
    crate::test_support::init_config();
    let game = config::configs::get();

    let replacement = resolve_feature(
        Some(game),
        "1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223",
    )
    .unwrap();
    assert!(replacement.arguments_supported);
    assert_eq!(replacement.references(Some(game)).skills.len(), 6);

    let assassination = resolve_feature(
        Some(game),
        "10004#10#1#312401451,312401452:312401453,312401454",
    )
    .unwrap();
    assert!(assassination.arguments_supported);
    assert_eq!(
        assassination.values,
        [10004, 10, 1, 312401451, 312401452, 312401453, 312401454]
    );
    assert!(assassination.references(Some(game)).skills.is_empty());
    assert!(assassination.references(Some(game)).buffs.is_empty());

    let malformed = resolve_feature(Some(game), "10004#10#1#312401451,312401452:bad").unwrap();
    assert!(malformed.values.is_empty());
    assert!(malformed.is_malformed());

    let raspberry =
        resolve_feature(Some(game), "1042#100#100#700#150#203#50#211#33#1#40#26.4").unwrap();
    assert!(raspberry.arguments_supported);
    assert_eq!(raspberry.values.last(), Some(&264));
}
