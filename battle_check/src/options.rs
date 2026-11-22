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
    pub(crate) skill_ids: Vec<i32>,
    pub(crate) buff_ids: Vec<i32>,
    pub(crate) episode_id: Option<i32>,
    pub(crate) psychube_id: Option<i32>,
    pub(crate) psychube_level: Option<i32>,
    pub(crate) destiny_stone: Option<i32>,
    pub(crate) destiny_rank: Option<i32>,
    pub(crate) coverage_plan: bool,
    pub(crate) include_plan: bool,
    pub(crate) simulate_opening: bool,
    pub(crate) explain: bool,
    pub(crate) json: bool,
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
                    "battle_check [--hero ID]... [--skill ID]... [--buff ID]... [--episode ID] [--explain] [--json]\n\
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
            "--json" => {
                options.json = true;
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
            "--hero" | "--skill" | "--buff" | "--episode" | "--psychube" | "--psychube-level"
            | "--destiny-stone" | "--destiny-rank" => args
                .next()
                .with_context(|| format!("missing value for {arg}"))?,
            _ => bail!("unknown argument {arg}"),
        };
        let value = value
            .parse::<i32>()
            .with_context(|| format!("invalid value for {arg}: {value}"))?;
        match arg.as_str() {
            "--hero" => options.hero_ids.push(value),
            "--skill" => options.skill_ids.push(value),
            "--buff" => options.buff_ids.push(value),
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
    if options
        .skill_ids
        .iter()
        .chain(&options.buff_ids)
        .any(|id| *id <= 0)
    {
        bail!("--skill and --buff IDs must be positive");
    }

    if let Some(kind) = options.battle_report {
        if options.coverage_plan || options.include_plan || options.simulate_opening || options.json
        {
            bail!(
                "--battle-report cannot be combined with --coverage-plan, --include-plan, --simulate-opening, or --json"
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
        let has_skill = !options.skill_ids.is_empty();
        let has_buff = !options.buff_ids.is_empty();
        let has_psychube = options.psychube_id.is_some();
        let has_episode = options.episode_id.is_some();
        match kind {
            ReportKind::All if has_hero || has_skill || has_buff || has_psychube || has_episode => {
                bail!("--battle-report all does not accept narrowing selectors")
            }
            ReportKind::Hero if has_skill || has_buff || has_psychube || has_episode => {
                bail!("--battle-report hero accepts --hero but not --psychube or --episode")
            }
            ReportKind::Psychube if has_hero || has_skill || has_buff || has_episode => {
                bail!("--battle-report psychube accepts --psychube but not --hero or --episode")
            }
            ReportKind::Stage if has_hero || has_skill || has_buff || has_psychube => {
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
        && options.skill_ids.is_empty()
        && options.buff_ids.is_empty()
        && options.episode_id.is_none()
        && !options.coverage_plan
        && !options.include_plan
    {
        bail!("provide --hero, --skill, --buff, --episode, --coverage-plan, or --include-plan");
    }
    if options.json && (options.coverage_plan || options.include_plan || options.simulate_opening) {
        bail!("--json is only supported for closure scans");
    }
    if (options.coverage_plan || options.include_plan)
        && (options.episode_id.is_some()
            || !options.skill_ids.is_empty()
            || !options.buff_ids.is_empty()
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
mod tests;
