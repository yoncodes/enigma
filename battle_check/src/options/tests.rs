use super::*;

#[test]
fn parses_optional_build_levels() {
    let options = parse_args(
        [
            "--hero",
            "3086",
            "--psychube",
            "1527",
            "--psychube-level",
            "4",
            "--destiny-stone",
            "308601",
            "--destiny-rank",
            "3",
            "--explain",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .unwrap();

    assert_eq!(options.hero_ids, vec![3086]);
    assert_eq!(options.psychube_id, Some(1527));
    assert_eq!(options.psychube_level, Some(4));
    assert_eq!(options.destiny_stone, Some(308601));
    assert_eq!(options.destiny_rank, Some(3));
    assert!(options.explain);
}

#[test]
fn parses_capture_roots_without_treating_them_as_numeric_values() {
    let root = std::env::temp_dir().join(format!(
        "enigma-battle-check-options-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let options = parse_args(
        ["--hero", "3127", "--capture-root", root.to_str().unwrap()]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();

    assert_eq!(options.capture_roots, vec![root.clone()]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_missing_capture_root() {
    let error = parse_args(
        ["--hero", "3127", "--capture-root", "missing-capture-root"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap_err();

    assert!(error.to_string().contains("not a readable directory"));
}

#[test]
fn accepts_rosters_and_focused_coverage() {
    let roster = parse_args(
        ["--hero", "3120", "--hero", "3125"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(roster.hero_ids, vec![3120, 3125]);

    let focused = parse_args(
        ["--coverage-plan", "--hero", "3127"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();
    assert!(focused.coverage_plan);
    assert_eq!(focused.hero_ids, vec![3127]);

    let include_plan = parse_args(
        ["--include-plan", "--hero", "3117"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();
    assert!(include_plan.include_plan);
    assert_eq!(include_plan.hero_ids, vec![3117]);

    let simulation = parse_args(
        ["--episode", "10002", "--simulate-opening"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();
    assert!(simulation.simulate_opening);
}

#[test]
fn accepts_machine_readable_skill_and_buff_roots() {
    let options = parse_args(
        ["--skill", "116371018", "--buff", "116371017", "--json"]
            .into_iter()
            .map(str::to_owned),
    )
    .unwrap();

    assert_eq!(options.skill_ids, vec![116371018]);
    assert_eq!(options.buff_ids, vec![116371017]);
    assert!(options.json);
}

#[test]
fn rejects_empty_skill_and_buff_roots() {
    for args in [["--skill", "0", "--json"], ["--buff", "0", "--json"]] {
        assert!(parse_args(args.into_iter().map(str::to_owned)).is_err());
    }
}

#[test]
fn parses_report_kinds_and_matching_selectors() {
    let cases = [
        (vec!["all"], ReportKind::All),
        (vec!["hero", "--hero", "3086"], ReportKind::Hero),
        (vec!["psychube", "--psychube", "1527"], ReportKind::Psychube),
        (vec!["stage", "--episode", "10002"], ReportKind::Stage),
    ];

    for (selectors, kind) in cases {
        let mut args = vec!["--battle-report", selectors[0], "--report-dir", "reports"];
        args.extend_from_slice(&selectors[1..]);
        let options = parse_args(args.into_iter().map(str::to_owned)).unwrap();

        assert_eq!(options.battle_report, Some(kind));
        assert_eq!(options.report_dir, Some(PathBuf::from("reports")));
    }
}

#[test]
fn report_mode_accepts_capture_roots() {
    let root = std::env::temp_dir().join(format!(
        "enigma-battle-check-report-options-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let options = parse_args(
        [
            "--battle-report",
            "all",
            "--report-dir",
            "reports",
            "--capture-root",
            root.to_str().unwrap(),
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .unwrap();

    assert_eq!(options.capture_roots, vec![root.clone()]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_report_pairing_and_mismatched_selectors() {
    for args in [
        vec!["--battle-report", "hero"],
        vec!["--report-dir", "reports", "--hero", "3086"],
    ] {
        assert!(parse_args(args.into_iter().map(str::to_owned)).is_err());
    }

    for args in [
        vec![
            "--battle-report",
            "all",
            "--report-dir",
            "reports",
            "--hero",
            "3086",
        ],
        vec![
            "--battle-report",
            "hero",
            "--report-dir",
            "reports",
            "--episode",
            "10002",
        ],
        vec![
            "--battle-report",
            "psychube",
            "--report-dir",
            "reports",
            "--hero",
            "3086",
        ],
        vec![
            "--battle-report",
            "psychube",
            "--report-dir",
            "reports",
            "--episode",
            "10002",
        ],
        vec![
            "--battle-report",
            "stage",
            "--report-dir",
            "reports",
            "--hero",
            "3086",
        ],
    ] {
        assert!(parse_args(args.into_iter().map(str::to_owned)).is_err());
    }
}
