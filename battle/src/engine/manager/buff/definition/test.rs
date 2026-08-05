use super::*;

#[test]
fn parses_include_type_values_like_csharp() {
    assert_eq!(
        parse_include_entries("10#3|11#2,7#1"),
        Ok(vec![(10, 3), (11, 2), (7, 1)])
    );
    assert_eq!(parse_include_entries("1|10#3"), Ok(vec![(1, 0), (10, 3)]));
    assert!(parse_include_entries("1#10").is_err());
}

#[test]
fn parses_exact_exclude_buff_ids() {
    assert_eq!(
        parse_exclude_buff_ids("1#9|2#530000111,530000112"),
        vec![530000111, 530000112]
    );
    assert_eq!(
        parse_exclude_buff_ids("2#530000111\u{ff0c}530000112"),
        vec![530000111, 530000112]
    );
}

#[test]
fn halo_fanout_keeps_add_markers_from_non_halo_features() {
    crate::test_support::init_config();

    for buff_id in [312401461, 312441464, 312441465] {
        assert_eq!(
            BuffDefinition::get(buff_id)
                .unwrap()
                .fanout_wire_markers(crate::engine::skill::buff_act::wire::WirePhase::Add,),
            vec![sonettobuf::effect_type_enum::EffectType::Attr as i32]
        );
    }
}

#[test]
fn initializes_common_params_for_stateful_buff_acts() {
    crate::test_support::init_config();

    assert_eq!(initial_act_common_params("772#1|806#5"), "806#0");
    assert_eq!(
        initial_act_common_params("1004#205#500#500#228003"),
        "1004#0"
    );
    assert_eq!(initial_act_common_params("1003#1#228004#6"), "1003#0");
    assert_eq!(initial_act_common_params("10000"), "10000#1,0,0");
}

#[test]
fn stacked_markers_use_the_layer_child_uid_lane() {
    crate::test_support::init_config();

    assert!(BuffDefinition::get(31250191).unwrap().uses_child_uid());
    assert!(BuffDefinition::get(31200142).unwrap().uses_child_uid());
    assert!(BuffDefinition::get(31260171).unwrap().uses_child_uid());
    assert!(BuffDefinition::get(31260161).unwrap().uses_child_uid());
}

#[test]
fn visible_layered_attribute_buff_has_no_post_apply_uid_reservation() {
    crate::test_support::init_config();

    let higge = BuffDefinition::get(31200142).unwrap();
    assert!(higge.uses_child_uid());
    assert!(!higge.reserves_child_after_first_apply());

    let lucy_upgrade = BuffDefinition::get(30860113).unwrap();
    assert!(lucy_upgrade.uses_child_uid());
    assert!(lucy_upgrade.reserves_child_after_first_apply());
}

#[test]
fn maps_real_harm_fix_to_genesis_damage_bonus() {
    crate::test_support::init_config();

    assert_eq!(
        parse_attribute_deltas("522#70"),
        vec![(AttrId::GenesisDmgBonus, 70)]
    );
}

#[test]
fn initial_wire_state_comes_from_the_resolved_exact_feature() {
    crate::test_support::init_config();

    let crystal = BuffDefinition::get(31340008)
        .unwrap()
        .initial_wire_states(10, 20, 1, 1000);
    assert_eq!(crystal[0].act_id, 1049);
    assert_eq!(crystal[0].params, vec![2, 2, 0]);
    assert_eq!(crystal[0].team_type, 1);

    let kill = BuffDefinition::get(31280111)
        .unwrap()
        .initial_wire_states(10, 21, 1, 1000);
    assert_eq!(kill[0].act_id, 1028);
    assert_eq!(kill[0].str_param.as_deref(), Some("200"));
}

#[test]
fn conduit_selection_initial_state_advertises_configured_options() {
    crate::test_support::init_config();

    let markers = BuffDefinition::get(31490013)
        .unwrap()
        .initial_wire_states(10, 1146, 1, 1000);

    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].act_id, 10030);
    assert_eq!(
        markers[0].params,
        [
            0, 1, 31495201, 31495211, 31446012, 31446022, 31446013, 31446023
        ]
    );
}

#[test]
fn butterfly_record_initial_state_advertises_configured_skill_kinds() {
    crate::test_support::init_config();

    let markers = BuffDefinition::get(235002)
        .unwrap()
        .initial_wire_states(-1, 1042, 1, 1000);

    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].act_id, 1104);
    assert_eq!(markers[0].params, vec![1, 2, 3, 4, 5, 6, 9, 13]);
    assert_eq!(markers[0].str_param.as_deref(), Some("3,0,0"));
    assert_eq!(markers[0].team_type, 1);
}

#[test]
fn stacked_include_value_is_the_layer_cap() {
    let definition = BuffDefinition {
        id: 1,
        type_id: 1,
        group: 0,
        is_no_show: false,
        status_id: 0,
        status: BuffStatus::Unknown,
        duration: 0,
        count: 0,
        exclude_buff_ids: Vec::new(),
        exclude_status_ids: Vec::new(),
        include_entries: parse_include_entries("10#3").unwrap(),
        include_types_valid: true,
        attribute_deltas: Vec::new(),
        features: Vec::new(),
        has_features: false,
        act_common_params: String::new(),
        take_stage: 0,
        take_act: String::new(),
    };

    assert_eq!(definition.stack_max_layer(), 3);
    assert_eq!(definition.cap_layer(5), 3);
}

#[test]
fn parses_attribute_features_once() {
    assert_eq!(
        parse_attribute_deltas("100#102#10|926#1|100#205#15"),
        vec![(AttrId::Attack, 10), (AttrId::DmgBonus, 15)]
    );
}

#[test]
fn parses_distinct_owner_cast_and_attacked_skill_slot_rules() {
    use super::super::{BuffTakeAction, lifecycle::BuffTakeActionTrigger};

    assert_eq!(
        BuffTakeAction::parse("9#1，2，3"),
        Some(BuffTakeAction {
            trigger: BuffTakeActionTrigger::OwnerCastSkill,
            skill_slots: vec![1, 2, 3],
        })
    );
    assert_eq!(
        BuffTakeAction::parse("10#3"),
        Some(BuffTakeAction {
            trigger: BuffTakeActionTrigger::OwnerAttackedBySkill,
            skill_slots: vec![3],
        })
    );
    assert_eq!(BuffTakeAction::parse("12#3"), None);
}
