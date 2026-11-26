use super::*;

#[cfg(windows)]
fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, destination)
}

#[cfg(unix)]
fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

#[cfg(windows)]
fn link_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(source, destination)
}

#[cfg(unix)]
fn link_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

#[test]
fn nested_wrapper_supplies_the_embedded_fight_identity() {
    let root = std::env::temp_dir().join(format!("enigma-wire-evidence-{}", std::process::id()));
    let path = root.join("tower");
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("StartTowerBattleReply.json"),
        r#"{"startDungeonReply":{"fight":{"version":7,"episodeId":10,"battleId":20}}}"#,
    )
    .unwrap();

    assert_eq!(fight_identity(&path), Ok(Some((7, 10, 20))));
    assert_eq!(
        response_files(&root),
        vec![
            path.join("StartTowerBattleReply.json")
                .canonicalize()
                .unwrap()
        ]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_ambiguous_capture_directories() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-ambiguous-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    for (name, episode_id) in [
        ("StartDungeonReply.json", 10),
        ("StartDungeonReply_2.json", 11),
    ] {
        fs::write(
            root.join(name),
            format!(r#"{{"fight":{{"version":7,"episodeId":{episode_id},"battleId":20}}}}"#),
        )
        .unwrap();
    }

    assert_eq!(fight_identity(&root), Err(()));
    let response = root.join("ZZ_BeginRoundReply.json");
    fs::write(&response, "{}").unwrap();
    assert_eq!(response_identity(&response), Err(()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn flat_capture_stream_uses_latest_preceding_setup() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-flat-stream-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    for (name, episode_id) in [
        ("20261031_100000_StartDungeonReply.json", 10),
        ("20261031_110000_StartDungeonReply.json", 11),
    ] {
        fs::write(
            root.join(name),
            format!(r#"{{"fight":{{"version":7,"episodeId":{episode_id},"battleId":20}}}}"#),
        )
        .unwrap();
    }
    let response = root.join("20261031_100100_BeginRoundReply.json");
    fs::write(&response, "{}").unwrap();
    let auto = root.join("20261031_100200_AutoRoundReply.json");
    fs::write(&auto, "{}").unwrap();

    assert_eq!(response_identity(&response), Ok(Some((7, 10, 20))));
    assert!(response_files(&root).contains(&auto.canonicalize().unwrap()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unreadable_setup_preserves_its_session_boundary() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-unreadable-boundary-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("20261031_100000_StartDungeonReply.json"),
        r#"{"fight":{"version":7,"episodeId":10,"battleId":20}}"#,
    )
    .unwrap();
    fs::write(
        root.join("20261031_110000_StartDungeonReply.json"),
        "not json",
    )
    .unwrap();
    let response = root.join("20261031_110100_BeginRoundReply.json");
    fs::write(&response, "{}").unwrap();

    assert_eq!(response_identity(&response), Err(()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bounded_walk_does_not_follow_directory_links() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-link-root-{}",
        std::process::id()
    ));
    let outside = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-link-outside-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("StartDungeonReply.json"), "{}").unwrap();
    let link = root.join("escape");
    if link_directory(&outside, &link).is_ok() {
        assert!(capture_json_files(&root).is_empty());
        fs::remove_dir(&link).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn broad_capture_root_uses_only_decoded_json() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-decoded-root-{}",
        std::process::id()
    ));
    let decoded = root.join("pcap/decoded/common");
    let config = root.join("part1/json_dump/configs/excel2json");
    fs::create_dir_all(&decoded).unwrap();
    fs::create_dir_all(&config).unwrap();
    fs::write(decoded.join("GetFightReply.json"), "{}").unwrap();
    fs::write(config.join("skill_effect.json"), "{}").unwrap();

    assert_eq!(
        capture_json_files(&root),
        vec![decoded.join("GetFightReply.json").canonicalize().unwrap()]
    );
    assert_eq!(
        capture_json_files(&decoded),
        vec![decoded.join("GetFightReply.json").canonicalize().unwrap()]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn linked_decoded_directory_does_not_enable_static_fallback() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-linked-decoded-root-{}",
        std::process::id()
    ));
    let outside = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-linked-decoded-outside-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(root.join("skill_effect.json"), "{}").unwrap();
    fs::write(outside.join("GetFightReply.json"), "{}").unwrap();
    let link = root.join("decoded");
    if link_directory(&outside, &link).is_ok() {
        assert!(capture_json_files(&root).is_empty());
        fs::remove_dir(&link).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn fight_identity_ignores_linked_start_replies() {
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-linked-identity-root-{}",
        std::process::id()
    ));
    let outside = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-linked-identity-outside-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let source = outside.join("StartDungeonReply.json");
    fs::write(
        &source,
        r#"{"fight":{"version":7,"episodeId":10,"battleId":20}}"#,
    )
    .unwrap();
    let link = root.join("StartDungeonReply.json");
    if link_file(&source, &link).is_ok() {
        assert_eq!(fight_identity(&root), Ok(None));
        fs::remove_file(&link).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn only_version_seven_contributes_wire_evidence() {
    crate::init_config().unwrap();
    let root = std::env::temp_dir().join(format!(
        "enigma-wire-evidence-version-{}",
        std::process::id()
    ));
    let battle = root.join("battle");
    fs::create_dir_all(&battle).unwrap();
    let start = battle.join("StartDungeonReply.json");
    fs::write(
        battle.join("AutoRoundReply.json"),
        r#"{"fightStep":[{"actEffect":[{"effectType":5,"buff":{"buffId":31340003}},{"effectType":0}]}]}"#,
    )
    .unwrap();
    let write_start = |version| {
        fs::write(
            &start,
            format!(r#"{{"fight":{{"version":{version},"episodeId":10,"battleId":20}}}}"#),
        )
        .unwrap();
    };

    write_start(6);
    let evidence = Evidence::collect(config::configs::get(), std::slice::from_ref(&root));
    assert!(
        evidence
            .source(1052, "HeatScaleTag", WirePhase::Add, 0)
            .is_none()
    );

    write_start(7);
    let evidence = Evidence::collect(config::configs::get(), std::slice::from_ref(&root));
    assert!(
        evidence
            .source(1052, "HeatScaleTag", WirePhase::Add, 0)
            .is_some()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn runtime_buff_act_marker_uses_its_carrier_as_evidence() {
    crate::init_config().unwrap();
    let root = std::env::temp_dir().join(format!("enigma-runtime-wire-{}", std::process::id()));
    let battle = root.join("battle");
    fs::create_dir_all(&battle).unwrap();
    fs::write(
        battle.join("StartDungeonReply.json"),
        r#"{"fight":{"version":7,"episodeId":38540113,"battleId":116385406}}"#,
    )
    .unwrap();
    fs::write(
        battle.join("BeginRoundReply_1.json"),
        r#"{"fightStep":[{"actEffect":[{"buff":null,"buffActId":305,"effectNum":500110,"effectType":42}]}]}"#,
    )
    .unwrap();

    let evidence = Evidence::collect(config::configs::get(), std::slice::from_ref(&root));

    assert!(
        evidence
            .source(305, "AddToAttacker", WirePhase::Static, 42)
            .is_some()
    );
    assert_eq!(
        evidence.carrier_sources(305, "AddToAttacker", 500110),
        ["episode 38540113 / battle 116385406"]
    );
    assert!(
        evidence
            .carrier_sources(305, "AddToAttacker", 500111)
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reserve_cycle_wire_evidence_requires_the_configured_feature() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut features = configured_features(db, "931|931#1");
    let mut valid = features.next().unwrap();

    assert_eq!(valid.wire().unwrap().markers(WirePhase::Add), [0]);
    assert!(features.next().unwrap().wire().is_none());

    valid.effect_time = 0;
    assert!(valid.wire().is_none());
}

#[test]
fn reserve_cycle_evidence_matches_the_untagged_grant_recipient() {
    use serde_json::json;

    crate::init_config().unwrap();
    let db = config::get();
    let grant = json!({"effectType": 5, "targetId": "-2", "buff": {"buffId": 9260001}});
    for (marker, expected) in [
        (
            json!({"effectType": 0, "targetId": -2, "buffActId": 0, "configEffect": 0}),
            true,
        ),
        (json!({"effectType": 0, "targetId": "-3"}), false),
        (json!({"effectType": 0}), false),
        (
            json!({"effectType": 0, "targetId": "-2", "buffActId": 931, "effectNum": 9260001}),
            false,
        ),
        (
            json!({"effectType": 0, "targetId": "-2", "configEffect": 931}),
            false,
        ),
    ] {
        let mut evidence = Evidence::default();
        evidence.inspect_effects(&[grant.clone(), marker], db, "Limbo");
        assert_eq!(
            evidence
                .source(931, "CycleToSub", WirePhase::Add, 0)
                .is_some(),
            expected
        );
    }
}

#[cfg(feature = "private-fixtures")]
#[test]
fn derives_marker_evidence_from_captures() {
    use sonettobuf::effect_type_enum::EffectType;

    crate::init_config().unwrap();
    let evidence = Evidence::collect(config::configs::get(), &[]);

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../battle_preview/fixtures/battles");
    assert_eq!(
        fight_identity(&fixtures.join("battle75"))
            .ok()
            .flatten()
            .map(|identity| identity.0),
        Some(7)
    );
    assert!(
        evidence
            .source(
                112,
                "AttrOnlyCalDamageBeAttacked",
                WirePhase::Static,
                EffectType::Attr as i32,
            )
            .is_none()
    );
    for (phase, effect_type) in [
        (WirePhase::Add, EffectType::Redorbluecount as i32),
        (WirePhase::Refresh, EffectType::Redorbluecountchange as i32),
    ] {
        assert!(
            evidence
                .source(897, "RedOrBlueCount", phase, effect_type)
                .is_some()
        );
    }
}
