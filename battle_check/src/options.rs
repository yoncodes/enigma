use std::path::PathBuf;

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReportKind {
    All,
    Hero,
    Psychube,
    Stage,
}

#[derive(Debug, Default)]
pub(crate) struct Options {
    pub(crate) hero_ids: Vec<i32>,
    pub(crate) episode_id: Option<i32>,
    pub(crate) psychube_id: Option<i32>,
    pub(crate) psychube_level: Option<i32>,
    pub(crate) destiny_stone: Option<i32>,
    pub(crate) destiny_rank: Option<i32>,
    pub(crate) coverage_plan: bool,
    pub(crate) include_plan: bool,
    pub(crate) simulate_opening: bool,
    pub(crate) explain: bool,
    pub(crate) capture_roots: Vec<PathBuf>,
    pub(crate) battle_report: Option<ReportKind>,
    pub(crate) report_dir: Option<PathBuf>,
}

pub(crate) fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--help" | "-h" => {
                println!("battle_check --battle-report all|hero|psychube|stage --report-dir PATH");
                println!(
                    "battle_check [--hero ID]... [--episode ID] [--explain]\n\
                     battle_check --episode ID --simulate-opening\n\
                     battle_check --coverage-plan [--hero ID]...\n\
                     battle_check --include-plan [--hero ID]...\n\
                     [--capture-root PATH]...\n\
                     [--psychube ID --psychube-level LEVEL]\n\
                     [--destiny-stone ID --destiny-rank RANK]\n\
                     Hero upgrades are checked at maximum rank; selected equipment uses the requested level."
                );
                std::process::exit(0);
            }
            "--explain" => {
                options.explain = true;
                continue;
            }
            "--coverage-plan" => {
                options.coverage_plan = true;
                continue;
            }
            "--include-plan" => {
                options.include_plan = true;
                continue;
            }
            "--simulate-opening" => {
                options.simulate_opening = true;
                continue;
            }
            "--capture-root" => {
                let path = PathBuf::from(
                    args.next()
                        .with_context(|| format!("missing value for {arg}"))?,
                );
                if !path.is_dir() {
                    bail!(
                        "capture root is not a readable directory: {}",
                        path.display()
                    );
                }
                options.capture_roots.push(path);
                continue;
            }
            "--battle-report" => {
                let kind = args
                    .next()
                    .with_context(|| format!("missing value for {arg}"))?;
                options.battle_report = Some(match kind.as_str() {
                    "all" => ReportKind::All,
                    "hero" => ReportKind::Hero,
                    "psychube" => ReportKind::Psychube,
                    "stage" => ReportKind::Stage,
                    _ => bail!(
                        "invalid value for --battle-report: {kind}; expected all, hero, psychube, or stage"
                    ),
                });
                continue;
            }
            "--report-dir" => {
                options.report_dir = Some(PathBuf::from(
                    args.next()
                        .with_context(|| format!("missing value for {arg}"))?,
                ));
                continue;
            }
            "--hero" | "--episode" | "--psychube" | "--psychube-level" | "--destiny-stone"
            | "--destiny-rank" => args
                .next()
                .with_context(|| format!("missing value for {arg}"))?,
            _ => bail!("unknown argument {arg}"),
        };
        let value = value
            .parse::<i32>()
            .with_context(|| format!("invalid value for {arg}: {value}"))?;
        match arg.as_str() {
            "--hero" => options.hero_ids.push(value),
            "--episode" => options.episode_id = Some(value),
            "--psychube" => options.psychube_id = Some(value),
            "--psychube-level" => options.psychube_level = Some(value),
            "--destiny-stone" => options.destiny_stone = Some(value),
            "--destiny-rank" => options.destiny_rank = Some(value),
            _ => unreachable!(),
        }
    }

    match (options.battle_report, options.report_dir.is_some()) {
        (Some(_), false) => bail!("--battle-report requires --report-dir"),
        (None, true) => bail!("--report-dir requires --battle-report"),
        _ => {}
    }

    if options.destiny_stone.is_some() != options.destiny_rank.is_some() {
        bail!("--destiny-stone and --destiny-rank must be provided together");
    }

    if let Some(kind) = options.battle_report {
        if options.coverage_plan || options.include_plan || options.simulate_opening {
            bail!(
                "--battle-report cannot be combined with --coverage-plan, --include-plan, or --simulate-opening"
            );
        }
        if options.destiny_stone.is_some() {
            bail!("destiny selection is not supported with --battle-report");
        }
        if options.psychube_level.is_some() {
            bail!(
                "--battle-report psychube scans the maximum configured rank; omit --psychube-level"
            );
        }

        let has_hero = !options.hero_ids.is_empty();
        let has_psychube = options.psychube_id.is_some();
        let has_episode = options.episode_id.is_some();
        match kind {
            ReportKind::All if has_hero || has_psychube || has_episode => {
                bail!("--battle-report all does not accept narrowing selectors")
            }
            ReportKind::Hero if has_psychube || has_episode => {
                bail!("--battle-report hero accepts --hero but not --psychube or --episode")
            }
            ReportKind::Psychube if has_hero || has_episode => {
                bail!("--battle-report psychube accepts --psychube but not --hero or --episode")
            }
            ReportKind::Stage if has_hero || has_psychube => {
                bail!("--battle-report stage accepts --episode but not --hero or --psychube")
            }
            _ => {}
        }
        return Ok(options);
    }

    if options.psychube_id.is_some() != options.psychube_level.is_some() {
        bail!("--psychube and --psychube-level must be provided together");
    }

    if options.hero_ids.is_empty()
        && options.episode_id.is_none()
        && !options.coverage_plan
        && !options.include_plan
    {
        bail!("provide --hero, --episode, --coverage-plan, --include-plan, or both");
    }
    if (options.coverage_plan || options.include_plan)
        && (options.episode_id.is_some()
            || options.psychube_id.is_some()
            || options.destiny_stone.is_some())
    {
        bail!(
            "--coverage-plan and --include-plan support optional heroes but not episodes or equipment selections"
        );
    }
    if options.simulate_opening && options.episode_id.is_none() {
        bail!("--simulate-opening requires --episode");
    }
    if options.hero_ids.is_empty()
        && (options.psychube_id.is_some()
            || options.psychube_level.is_some()
            || options.destiny_stone.is_some()
            || options.destiny_rank.is_some())
    {
        bail!("--psychube and --destiny-stone require --hero");
    }
    if options.hero_ids.len() > 1
        && (options.psychube_id.is_some() || options.destiny_stone.is_some())
    {
        bail!("equipment selection requires exactly one --hero");
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
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
}
