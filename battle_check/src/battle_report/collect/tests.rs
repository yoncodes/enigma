use super::*;

#[test]
fn removes_client_rich_text_without_joining_words() {
    assert_eq!(
        clean_rich_text("<i>Lady by the Lake</i>"),
        "Lady by the Lake"
    );
    assert_eq!(
        clean_rich_text("In [Silence<id:3136>],<br>gains <nobr>2 stacks</nobr>."),
        "In [Silence], gains 2 stacks."
    );
    assert_eq!(clean_rich_text("HP < 50%"), "HP < 50%");
}

#[test]
fn route_names_hide_config_placeholders() {
    assert_eq!(route_name("?".to_owned()), None);
    assert_eq!(route_name(String::new()), None);
    assert_eq!(
        route_name("Focused Flashback".to_owned()).as_deref(),
        Some("Focused Flashback")
    );
}

#[test]
fn condition_nodes_preserve_expression_and_exact_arguments() {
    let child = ParsedCondition {
        opcode: 999_999,
        type_name: "MissingExactCondition".to_owned(),
        kind: ParsedConditionKind::Unsupported("test".to_owned()),
        raw_args: vec!["1".to_owned()],
    };
    let wrapper = ParsedCondition {
        opcode: 0,
        type_name: "Any".to_owned(),
        kind: ParsedConditionKind::Any(vec![vec![child.clone()]]),
        raw_args: Vec::new(),
    };

    let conditions = [wrapper];
    let flattened = flatten_conditions(&conditions);
    assert_eq!(flattened, vec![&child]);
    let node = condition_node(flattened[0], "999999#1!|5", false);
    assert_eq!(node.raw, "999999#1!|5");
    assert_eq!(node.exact_raw, "999999#1");
}

#[test]
fn narrowed_reports_reject_unknown_subjects() {
    crate::init_config().unwrap();
    let db = config::configs::get();
    let evidence = Evidence::default();
    let wire_evidence = crate::wire_evidence::Evidence::default();

    assert!(
        heroes(
            db,
            battle::catalog::BattleCatalog::new(db),
            &[-1],
            &evidence,
            &wire_evidence,
        )
        .is_err()
    );
    assert!(
        psychubes(
            db,
            battle::catalog::BattleCatalog::new(db),
            Some(-1),
            &evidence,
            &wire_evidence,
        )
        .is_err()
    );
    assert!(
        stages(
            db,
            battle::catalog::BattleCatalog::new(db),
            Some(-1),
            &evidence,
            &wire_evidence,
        )
        .is_err()
    );
}

#[test]
fn grouped_skill_replacement_reports_raw_aware_semantic_support() {
    crate::init_config().unwrap();
    let db = config::get();
    let wire_evidence = crate::wire_evidence::Evidence::default();

    let supported = buff_act(
        db,
        0,
        "1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223",
        &wire_evidence,
    )
    .unwrap();
    assert_eq!(supported.node.semantic, "supported");

    for raw in [
        "1138#1:31460211#2:31460221",
        "1138#1:30120111,30120112,30120113#2:30120121,30120122,30120123",
    ] {
        assert_eq!(
            buff_act(db, 0, raw, &wire_evidence).unwrap().node.semantic,
            "unsupported arguments",
            "{raw}"
        );
    }
}

#[test]
fn charge_report_preserves_state_owner_and_reports_support() {
    crate::init_config().unwrap();
    let db = config::get();
    let wire_evidence = crate::wire_evidence::Evidence::default();

    let charge = buff_act(db, 0, "1139#100000#150000#31460183", &wire_evidence).unwrap();

    assert_eq!(charge.node.registry, "exact");
    assert_eq!(charge.node.semantic, "supported");
    assert_eq!(charge.destination, "StateConsumer");
}

#[test]
fn carrier_references_do_not_claim_marker_confirmation() {
    crate::init_config().unwrap();
    let root = std::env::temp_dir().join(format!("enigma-report-carriers-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("StartDungeonReply.json"),
        r#"{"fight":{"version":7,"episodeId":10,"battleId":20}}"#,
    )
    .unwrap();
    std::fs::write(root.join("BeginRoundReply.json"),
        r#"{"fightStep":[{"actEffect":[{"effectType":5,"buff":{"buffId":11440011}},{"effectType":5,"buff":{"buffId":31340003}},{"effectType":0}]}]}"#).unwrap();
    let db = config::get();
    let references = Evidence::collect(std::slice::from_ref(&root));
    let markers = crate::wire_evidence::Evidence::collect(db, std::slice::from_ref(&root));
    let report = buff(db, 11440011, &references, &markers).unwrap();
    let missing = report
        .acts
        .iter()
        .find(|act| act.node.opcode == Some(711))
        .unwrap();
    assert_eq!(missing.node.registry, "missing");
    assert_eq!(missing.node.semantic, "route missing");
    assert_eq!(missing.node.observation, "Carrier buff referenced");
    assert_eq!(
        missing.node.capture_sources,
        references.buff_sources(11440011)
    );
    assert!(missing.node.capture_sources[0].ends_with("BeginRoundReply.json"));

    let other = buff(db, 11410101, &references, &markers).unwrap();
    assert!(
        other
            .acts
            .iter()
            .all(|act| act.node.observation == "Unexercised")
    );
    let confirmed = buff(db, 31340003, &references, &markers).unwrap();
    let marker = confirmed
        .acts
        .iter()
        .find(|act| act.node.opcode == Some(1052))
        .unwrap();
    assert_eq!(marker.node.observation, "Exact wire marker observed");
    assert_eq!(marker.node.capture_sources, ["episode 10 / battle 20"]);
    std::fs::remove_dir_all(root).unwrap();
}
