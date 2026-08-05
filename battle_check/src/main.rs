use std::{collections::VecDeque, env, path::Path};

use anyhow::{Context, Result};
use battle::engine::skill::effect::SkillEffectCatalog;

mod coverage;
mod opening;
mod options;
mod scan;
mod wire_evidence;

fn main() -> Result<()> {
    let options = options::parse_args(env::args().skip(1))?;
    init_config()?;
    let db = config::configs::get();
    if options.coverage_plan || options.include_plan {
        let mut catalog = SkillEffectCatalog::from_game_db(db);
        return coverage::print_coverage_plan(
            db,
            &mut catalog,
            &options.hero_ids,
            options.include_plan.then_some("buff-include"),
            &options.capture_roots,
        );
    }
    let mut report = scan::Report {
        explain: options.explain,
        wire_evidence: wire_evidence::Evidence::collect(db, &options.capture_roots),
        ..Default::default()
    };
    let mut skills = VecDeque::new();
    let mut buffs = VecDeque::new();

    for &hero_id in &options.hero_ids {
        scan::collect_hero_roots(&options, hero_id, db, &mut skills, &mut report)?;
        println!("hero={hero_id} build=max");
    }
    if let Some(episode_id) = options.episode_id {
        scan::collect_episode_roots(episode_id, db, &mut skills, &mut report)?;
        println!("episode={episode_id}");
        if options.simulate_opening {
            opening::print(episode_id)?;
        }
    }

    let mut catalog = SkillEffectCatalog::from_roots(db, skills.iter().map(|skill| skill.id), []);
    scan::scan_closure(db, &mut catalog, &mut skills, &mut buffs, &mut report);
    report.print();
    if !report.is_ready() {
        std::process::exit(1);
    }
    Ok(())
}

fn init_config() -> Result<()> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json");
    config::init(data.to_str().unwrap()).context("load battle config")
}
