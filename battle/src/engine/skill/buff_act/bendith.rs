use crate::engine::skill::rule::RuleReferences;
use crate::engine::{
    event::payload::BattleEvent,
    manager::card::{CardCommand, CardReplaceOwnerSkills},
    manager::{BattleManagers, buff::ActiveBuffFeature},
    skill::{
        buff_act::registry::{BuffActKind, ParsedBuffAct},
        rule::output::{BattleCommand, RuleOp},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillGroupMapping {
    pub group1: Vec<i32>,
    pub group2: Vec<i32>,
}

pub fn parse_skill_group_mapping(raw: &str) -> Option<SkillGroupMapping> {
    let mut parts = raw.split('#');
    parts.next()?.trim().parse::<i32>().ok()?;
    parse_skill_group_parts(parts)
}

fn parse_skill_group_parts<'a>(
    parts: impl IntoIterator<Item = &'a str>,
) -> Option<SkillGroupMapping> {
    let mut group1 = None;
    let mut group2 = None;
    for part in parts {
        let (group, skills) = part.split_once(':')?;
        let skills = skills
            .split(',')
            .map(str::trim)
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if skills.len() != 3 || skills.iter().any(|skill_id| *skill_id <= 0) {
            return None;
        }
        match group.trim() {
            "1" if group1.is_none() => group1 = Some(skills),
            "2" if group2.is_none() => group2 = Some(skills),
            _ => return None,
        }
    }

    Some(SkillGroupMapping {
        group1: group1?,
        group2: group2?,
    })
}

pub fn replacement_skill_ids(raw: &str) -> Option<Vec<i32>> {
    let mapping = parse_skill_group_mapping(raw)?;
    Some(mapping.group1.into_iter().chain(mapping.group2).collect())
}

pub fn parse_feature(raw_args: &[String]) -> Option<Vec<i32>> {
    let mut group1 = None;
    let mut group2 = None;
    for raw in raw_args {
        let (group, skills) = raw.split_once(':')?;
        let skills = skills
            .split(',')
            .map(str::trim)
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if skills.is_empty() || skills.iter().any(|skill_id| *skill_id <= 0) {
            return None;
        }
        match group.trim() {
            "1" if group1.is_none() => group1 = Some(skills),
            "2" if group2.is_none() => group2 = Some(skills),
            _ => return None,
        }
    }
    Some(group1?.into_iter().chain(group2?).collect())
}

pub fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    RuleReferences {
        skills: feature.values.get(1..).unwrap_or_default().to_vec(),
        ..RuleReferences::default()
    }
}

pub fn supports_replace_entity_skill_group(game: Option<&config::GameDB>, raw: &str) -> bool {
    let (Some(game), Some(replacement)) = (game, parse_skill_group_mapping(raw)) else {
        return false;
    };
    base_skill_groups_from(game, &replacement).is_some()
}

pub fn replace_entity_skill_group_transaction(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let added = match event {
        BattleEvent::BuffAdded(_) => true,
        BattleEvent::BuffRemoved(_) => false,
        _ => return Vec::new(),
    };
    super::changed_features(managers, event, BuffActKind::ReplaceEntitySkillGroup)
        .into_iter()
        .filter_map(|(feature, _)| {
            let replacement = parse_skill_group_mapping(&feature.raw)?;
            let base = base_skill_groups(managers, &replacement)?;
            let origin = super::feature_command_origin(&feature)?;
            let owner_uid = feature.owner_uid;
            let (base, replacement) = if added {
                (base, replacement)
            } else {
                (replacement, base)
            };
            Some((
                feature,
                RuleOp::Command(BattleCommand::Card(CardCommand::ReplaceOwnerSkills(
                    CardReplaceOwnerSkills {
                        origin,
                        owner_uid,
                        base_group1: base.group1,
                        base_group2: base.group2,
                        replacement_group1: replacement.group1,
                        replacement_group2: replacement.group2,
                    },
                ))),
            ))
        })
        .collect()
}

fn base_skill_groups(
    managers: &BattleManagers,
    replacement: &SkillGroupMapping,
) -> Option<SkillGroupMapping> {
    base_skill_groups_from(managers.catalog().game_data(), replacement)
}

fn base_skill_groups_from(
    game: &config::GameDB,
    replacement: &SkillGroupMapping,
) -> Option<SkillGroupMapping> {
    let rows = &game.hero_upgrade_breaklevel;
    let resolve = |skills: &[i32]| {
        skills
            .iter()
            .map(|skill_id| {
                let mut matches = rows.iter().filter(|row| row.upgrade_skill_id == *skill_id);
                let base = matches.next()?.skill_id;
                matches.next().is_none().then_some(base)
            })
            .collect::<Option<Vec<_>>>()
    };
    Some(SkillGroupMapping {
        group1: resolve(&replacement.group1)?,
        group2: resolve(&replacement.group2)?,
    })
}

#[cfg(test)]
mod tests;
