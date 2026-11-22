use super::*;

#[test]
fn exhausted_effect_count_buff_keeps_packet_state_but_stops_contributing() {
    crate::test_support::init_config();
    let buff = |count| BuffInfo {
        uid: Some(3),
        buff_id: Some(301),
        from_uid: Some(2),
        count: Some(count),
        ..Default::default()
    };

    let definition = BuffDefinition::get(301).unwrap();
    assert!(active_feature(config::try_get(), 1, 1, true, &buff(0), Some(&definition)).is_empty());
    assert_eq!(
        active_feature(config::try_get(), 1, 1, true, &buff(1), Some(&definition))[0].amount,
        1
    );
}

#[test]
fn feature_lists_keep_each_configured_integer() {
    let feature = resolve_features("879#1#300#1,2").remove(0);

    assert_eq!(feature.values, vec![879, 1, 300, 1, 2]);
}

#[test]
fn unsupported_feature_arguments_do_not_become_active() {
    crate::test_support::init_config();
    let mut definition = BuffDefinition::get(31_000_161).unwrap();
    definition.replace_features_for_test(resolve_features("832#-7"));
    let buff = BuffInfo {
        buff_id: Some(31_000_161),
        count: Some(1),
        layer: Some(1),
        ..Default::default()
    };

    assert!(active_feature(config::try_get(), 1, 1, true, &buff, Some(&definition)).is_empty());
}

#[test]
fn sub_buff_exposes_child_features_and_passive_links() {
    crate::test_support::init_config();
    let definition = BuffDefinition::get(31260151).unwrap();
    let buff = BuffInfo {
        uid: Some(7),
        buff_id: Some(31260151),
        from_uid: Some(10),
        layer: Some(1),
        count: Some(1),
        ..Default::default()
    };

    let features = active_feature(config::try_get(), 10, 1, true, &buff, Some(&definition));

    assert!(
        features
            .iter()
            .any(|feature| { feature.buff_id == 31260201 && feature.act_id() == Some(932) })
    );
    assert!(
        features
            .iter()
            .any(|feature| { feature.buff_id == 31260201 && feature.act_id() == Some(865) })
    );
    assert_eq!(
        passive_skill_links(config::try_get(), 10, definition.features(), 1),
        vec![BuffPassiveSkillLink {
            owner_uid: 10,
            runtime_target_uid: 10,
            skill_id: 31260181,
        }]
    );
}
