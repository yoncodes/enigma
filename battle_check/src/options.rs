use std::path::PathBuf;

use anyhow::{Context, Result, bail};

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
}

pub(crate) fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--help" | "-h" => {
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
    if options.psychube_id.is_some() != options.psychube_level.is_some() {
        bail!("--psychube and --psychube-level must be provided together");
    }
    if options.destiny_stone.is_some() != options.destiny_rank.is_some() {
        bail!("--destiny-stone and --destiny-rank must be provided together");
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
}
