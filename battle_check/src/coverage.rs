use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::Result;
use battle::engine::{manager::buff::BuffPolicy, skill::effect::SkillEffectCatalog};

use crate::{
    options::Options,
    scan::{
        CapabilityKey, Report, collect_battle_roots, collect_hero_roots,
        collect_tower_assist_boss_roots, scan_closure,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WitnessKind {
    Hero,
    Stage,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Witness {
    kind: WitnessKind,
    id: i32,
    battle_id: Option<i32>,
    tower_id: Option<i32>,
    name: String,
}

struct HeroReadiness {
    id: i32,
    name: String,
    ready: bool,
    skills: usize,
    buffs: usize,
    errors: usize,
    warnings: usize,
    gaps: usize,
}

impl Witness {
    fn label(&self) -> String {
        let kind = match self.kind {
            WitnessKind::Hero => "hero",
            WitnessKind::Stage => "stage",
        };
        format!("{kind} {} ({})", self.id, self.name)
    }
}

pub(crate) fn print_coverage_plan(
    db: &config::GameDB,
    catalog: &mut SkillEffectCatalog,
    focus_heroes: &[i32],
    domain_filter: Option<&str>,
) -> Result<()> {
    let mut gaps = BTreeMap::<CapabilityKey, BTreeSet<&'static str>>::new();
    let mut gap_paths = BTreeMap::<CapabilityKey, BTreeSet<String>>::new();
    let mut witnesses = BTreeMap::<CapabilityKey, BTreeSet<Witness>>::new();
    let mut uses = BTreeMap::<Witness, BTreeSet<CapabilityKey>>::new();
    let mut hero_readiness = Vec::new();
    let options = Options::default();
    let wire_evidence = crate::wire_evidence::Evidence::collect(db);

    if domain_filter == Some("buff-include") {
        for buff in db.skill_buff.iter() {
            if let Ok(policy) = BuffPolicy::try_for_buff_id(buff.id) {
                for &(include_type, value) in policy.unresolved_include_entries.iter() {
                    let key = CapabilityKey::new(
                        "buff-include",
                        include_type,
                        BuffPolicy::include_entry_name(include_type, value),
                    );
                    gaps.entry(key.clone())
                        .or_default()
                        .insert("UnresolvedIncludePolicy");
                    gap_paths.entry(key).or_default().insert(format!(
                        "config > buff {} ({})",
                        buff.id,
                        display_name(db, "", &buff.name, buff.id)
                    ));
                }
            }
        }
    }

    for hero in db.character.iter().filter(|hero| hero.is_online == "1") {
        let witness = Witness {
            kind: WitnessKind::Hero,
            id: hero.id,
            battle_id: None,
            tower_id: None,
            name: display_name(db, &hero.name_eng, &hero.name, hero.id),
        };
        let mut skills = VecDeque::new();
        let mut report = Report {
            wire_evidence: wire_evidence.clone(),
            ..Default::default()
        };
        collect_hero_roots(&options, hero.id, db, &mut skills, &mut report)?;
        scan_closure(db, catalog, &mut skills, &mut VecDeque::new(), &mut report);
        hero_readiness.push(HeroReadiness {
            id: hero.id,
            name: witness.name.clone(),
            ready: report.is_ready(),
            skills: report.checked_skills.len(),
            buffs: report.checked_buffs.len(),
            errors: report.errors.len(),
            warnings: report.warnings.len(),
            gaps: report.gaps.len(),
        });
        index_report(
            witness,
            report,
            &mut gaps,
            &mut gap_paths,
            &mut witnesses,
            &mut uses,
        );
    }

    let mut stages = BTreeMap::new();
    for episode in db.episode.iter() {
        for (battle_id, first_clear) in
            [(episode.battle_id, false), (episode.first_battle_id, true)]
        {
            if battle_id > 0 && db.battle.get(battle_id).is_some() {
                stages.entry(battle_id).or_insert((episode, first_clear));
            }
        }
    }
    for (battle_id, (episode, first_clear)) in stages {
        let tower_id = db
            .tower_boss_episode
            .iter()
            .find(|row| row.episode_id == episode.id)
            .map(|row| row.tower_id)
            .or_else(|| {
                db.tower_boss_teach
                    .iter()
                    .find(|row| row.episode_id == episode.id)
                    .map(|row| row.tower_id)
            });
        let mut name = display_name(db, &episode.name_en, &episode.name, episode.id);
        if name == episode.id.to_string()
            && let Some(tower_id) = tower_id
            && let Some(tower) = db
                .tower_boss
                .iter()
                .find(|tower| tower.tower_id == tower_id)
        {
            name = format!(
                "Tower {tower_id}: {}",
                display_name(db, &tower.name_en, &tower.name, tower_id)
            );
        }
        if first_clear {
            name.push_str(" [first clear]");
        }
        let witness = Witness {
            kind: WitnessKind::Stage,
            id: episode.id,
            battle_id: Some(battle_id),
            tower_id,
            name,
        };
        let mut skills = VecDeque::new();
        let mut report = Report {
            quiet: true,
            ..Default::default()
        };
        collect_battle_roots(episode.id, battle_id, db, &mut skills, &mut report)?;
        if let Some(tower_id) = tower_id {
            collect_tower_assist_boss_roots(tower_id, db, &mut skills)?;
        }
        scan_closure(db, catalog, &mut skills, &mut VecDeque::new(), &mut report);
        index_report(
            witness,
            report,
            &mut gaps,
            &mut gap_paths,
            &mut witnesses,
            &mut uses,
        );
    }

    if !focus_heroes.is_empty() {
        let focused = uses
            .iter()
            .filter(|(witness, _)| {
                witness.kind == WitnessKind::Hero && focus_heroes.contains(&witness.id)
            })
            .flat_map(|(_, keys)| keys)
            .filter(|key| gaps.contains_key(*key))
            .cloned()
            .collect::<BTreeSet<_>>();
        gaps.retain(|key, _| focused.contains(key));
    }
    if let Some(domain) = domain_filter {
        gaps.retain(|key, _| key.domain == domain);
    }
    println!(
        "{} gaps={} heroes={} stages={} focus={}",
        if domain_filter == Some("buff-include") {
            "INCLUDE COVERAGE"
        } else {
            "COVERAGE"
        },
        gaps.len(),
        uses.keys()
            .filter(|witness| witness.kind == WitnessKind::Hero)
            .count(),
        uses.keys()
            .filter(|witness| witness.kind == WitnessKind::Stage)
            .count(),
        if focus_heroes.is_empty() {
            "all".to_owned()
        } else {
            focus_heroes
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        },
    );
    let shown_heroes = hero_readiness
        .iter()
        .filter(|hero| focus_heroes.is_empty() || focus_heroes.contains(&hero.id))
        .collect::<Vec<_>>();
    println!(
        "HERO READINESS ready={} incomplete={}",
        shown_heroes.iter().filter(|hero| hero.ready).count(),
        shown_heroes.iter().filter(|hero| !hero.ready).count(),
    );
    for hero in shown_heroes {
        println!(
            "  hero {} ({}) status={} skills={} buffs={} errors={} warnings={} gaps={}",
            hero.id,
            hero.name,
            if hero.ready { "READY" } else { "INCOMPLETE" },
            hero.skills,
            hero.buffs,
            hero.errors,
            hero.warnings,
            hero.gaps,
        );
    }
    for (key, reasons) in &gaps {
        println!(
            "GAP {} {} {} reason={}",
            key.domain,
            key.opcode,
            key.type_name,
            reasons.iter().copied().collect::<Vec<_>>().join(", "),
        );
        let exact = witnesses.get(key).cloned().unwrap_or_default();
        print_witnesses(
            "  heroes",
            exact.iter().filter(|item| item.kind == WitnessKind::Hero),
        );
        print_witnesses(
            "  stages",
            exact.iter().filter(|item| item.kind == WitnessKind::Stage),
        );
        if let Some(paths) = gap_paths.get(key) {
            let focused_prefixes = focus_heroes
                .iter()
                .map(|id| format!("hero {id} >"))
                .collect::<Vec<_>>();
            let paths = paths
                .iter()
                .filter(|path| {
                    focused_prefixes.is_empty()
                        || focused_prefixes
                            .iter()
                            .any(|prefix| path.starts_with(prefix))
                })
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                println!(
                    "  paths: {}",
                    limited(paths.into_iter().map(String::as_str), 6)
                );
            }
        }
        let same_type = witnesses
            .iter()
            .filter(|(candidate, _)| {
                candidate.domain == key.domain
                    && candidate.type_name == key.type_name
                    && candidate.opcode != key.opcode
            })
            .flat_map(|(candidate, items)| {
                items
                    .iter()
                    .map(move |item| format!("{} via opcode {}", item.label(), candidate.opcode))
            })
            .collect::<BTreeSet<_>>();
        if !same_type.is_empty() {
            println!(
                "  same-type clues (not interchangeable): {}",
                limited(same_type.iter().map(String::as_str), 8)
            );
        }
    }

    let gap_keys = gaps.keys().cloned().collect::<BTreeSet<_>>();
    let (plan, uncovered) = greedy_stage_plan(&gap_keys, &uses);
    println!(
        "STAGE PLAN selected={} uncovered={}",
        plan.len(),
        uncovered.len()
    );
    for (stage, covered) in plan {
        let labels = covered.iter().map(capability_label).collect::<Vec<_>>();
        println!(
            "  {} covers={}: {}",
            stage.label(),
            covered.len(),
            limited(labels.iter().map(String::as_str), 8),
        );
        let prefix = format!(
            "episode {} > battle {} >",
            stage.id,
            stage.battle_id.unwrap_or_default()
        );
        let tower_prefix = stage.tower_id.map(|id| format!("tower {id} >"));
        let paths = covered
            .iter()
            .filter_map(|key| gap_paths.get(key))
            .flatten()
            .filter(|path| {
                path.starts_with(&prefix)
                    || tower_prefix
                        .as_ref()
                        .is_some_and(|prefix| path.starts_with(prefix))
            })
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            println!(
                "    trigger paths: {}",
                limited(paths.into_iter().map(String::as_str), 16)
            );
        }
    }
    if !uncovered.is_empty() {
        let labels = uncovered.iter().map(capability_label).collect::<Vec<_>>();
        println!(
            "  no exact stage witness: {}",
            limited(labels.iter().map(String::as_str), 12)
        );
    }
    Ok(())
}

fn index_report(
    witness: Witness,
    report: Report,
    gaps: &mut BTreeMap<CapabilityKey, BTreeSet<&'static str>>,
    gap_paths: &mut BTreeMap<CapabilityKey, BTreeSet<String>>,
    witnesses: &mut BTreeMap<CapabilityKey, BTreeSet<Witness>>,
    uses: &mut BTreeMap<Witness, BTreeSet<CapabilityKey>>,
) {
    for (key, reasons) in report.gaps {
        gaps.entry(key).or_default().extend(reasons);
    }
    for (key, paths) in report.gap_paths {
        gap_paths.entry(key).or_default().extend(paths);
    }
    for key in &report.capabilities {
        witnesses
            .entry(key.clone())
            .or_default()
            .insert(witness.clone());
    }
    uses.insert(witness, report.capabilities);
}

fn greedy_stage_plan(
    gaps: &BTreeSet<CapabilityKey>,
    uses: &BTreeMap<Witness, BTreeSet<CapabilityKey>>,
) -> (
    Vec<(Witness, BTreeSet<CapabilityKey>)>,
    BTreeSet<CapabilityKey>,
) {
    let mut uncovered = gaps.clone();
    let mut plan = Vec::new();
    loop {
        let mut candidates = uses
            .iter()
            .filter(|(witness, _)| witness.kind == WitnessKind::Stage)
            .map(|(witness, keys)| {
                (
                    witness,
                    keys.intersection(&uncovered)
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                )
            })
            .filter(|(_, covered)| !covered.is_empty())
            .collect::<Vec<_>>();
        candidates.sort_by(|(left, left_keys), (right, right_keys)| {
            right_keys
                .len()
                .cmp(&left_keys.len())
                .then_with(|| left.cmp(right))
        });
        let Some((witness, covered)) = candidates.into_iter().next() else {
            break;
        };
        for key in &covered {
            uncovered.remove(key);
        }
        plan.push((witness.clone(), covered));
    }
    (plan, uncovered)
}

fn print_witnesses<'a>(label: &str, items: impl Iterator<Item = &'a Witness>) {
    let labels = items.map(Witness::label).collect::<Vec<_>>();
    println!(
        "{label}: {}",
        if labels.is_empty() {
            "none".to_owned()
        } else {
            limited(labels.iter().map(String::as_str), 8)
        }
    );
}

fn limited<'a>(items: impl Iterator<Item = &'a str>, limit: usize) -> String {
    let items = items.collect::<Vec<_>>();
    let mut output = items
        .iter()
        .take(limit)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > limit {
        output.push_str(&format!(" … +{}", items.len() - limit));
    }
    output
}

fn capability_label(key: &CapabilityKey) -> String {
    format!("{} {} {}", key.domain, key.opcode, key.type_name)
}

fn display_name(db: &config::GameDB, primary: &str, fallback: &str, id: i32) -> String {
    [primary, fallback]
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .map(|value| db.language_en.get(value).unwrap_or(value).to_owned())
        .unwrap_or_else(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_stages_by_exact_key_coverage() {
        let keys = [
            CapabilityKey::new("condition", 1, "A"),
            CapabilityKey::new("condition", 2, "A"),
            CapabilityKey::new("buff-act", 3, "B"),
        ];
        let gaps = keys.iter().cloned().collect::<BTreeSet<_>>();
        let stage = |id| Witness {
            kind: WitnessKind::Stage,
            id,
            battle_id: None,
            tower_id: None,
            name: id.to_string(),
        };
        let uses = BTreeMap::from([
            (stage(10), [keys[0].clone(), keys[1].clone()].into()),
            (stage(20), [keys[1].clone(), keys[2].clone()].into()),
        ]);

        let (plan, uncovered) = greedy_stage_plan(&gaps, &uses);

        assert_eq!(
            plan.iter().map(|(stage, _)| stage.id).collect::<Vec<_>>(),
            vec![10, 20]
        );
        assert!(uncovered.is_empty());
    }
}
