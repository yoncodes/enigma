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
mod tests {
    use super::*;
    use crate::engine::event::payload::BuffChangeEvent;
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    #[test]
    fn parses_the_structured_group_mapping() {
        assert_eq!(
            parse_skill_group_mapping(
                "1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223"
            ),
            Some(SkillGroupMapping {
                group1: vec![31460211, 31460212, 31460213],
                group2: vec![31460221, 31460222, 31460223],
            })
        );
    }

    #[test]
    fn rejects_flattened_or_unsupported_mappings() {
        crate::test_support::init_config();
        for raw in [
            "1138#1#31460211,31460212,31460213#2:31460221,31460222,31460223",
            "1138#1:31460211,31460212,31460213",
            "1138#1:31460211,31460212#2:31460221,31460222,31460223",
            "1138#1:31460211,0,31460213#2:31460221,31460222,31460223",
            "1138#1:31460211,31460212,31460213#3:31460221,31460222,31460223",
        ] {
            assert!(
                !supports_replace_entity_skill_group(config::try_get(), raw),
                "{raw}"
            );
        }
    }

    #[test]
    fn structurally_valid_mapping_without_unique_inverse_is_unsupported() {
        crate::test_support::init_config();
        assert!(!supports_replace_entity_skill_group(
            config::try_get(),
            "1138#1:30120111,30120112,30120113#2:30120121,30120122,30120123"
        ));
    }

    #[test]
    fn retained_mapping_adds_and_removes_as_exact_inverse_commands() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![31460114, 31460115, 31460116],
                    skill_group2: vec![31460127, 31460128, 31460129],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let change = BuffChangeEvent {
            source_uid: 10,
            target_uid: 10,
            buff_uid: 20,
            buff_id: 31460137,
            before_amount: 0,
            after_amount: 1,
            act_id: 0,
            act_value: 0,
        };
        let command = |event| {
            let operations = replace_entity_skill_group_transaction(&managers, &event);
            let [
                (_, RuleOp::Command(BattleCommand::Card(CardCommand::ReplaceOwnerSkills(command)))),
            ] = operations.as_slice()
            else {
                panic!("expected one exact replacement command")
            };
            command.clone()
        };

        let added = command(BattleEvent::BuffAdded(change));
        assert_eq!(added.owner_uid, 10);
        assert_eq!(added.base_group1, vec![31460114, 31460115, 31460116]);
        assert_eq!(added.replacement_group1, vec![31460214, 31460215, 31460216]);
        assert_eq!(added.base_group2, vec![31460127, 31460128, 31460129]);
        assert_eq!(added.replacement_group2, vec![31460227, 31460228, 31460229]);

        let removed = command(BattleEvent::BuffRemoved(BuffChangeEvent {
            before_amount: 1,
            after_amount: 0,
            ..change
        }));
        assert_eq!(removed.base_group1, added.replacement_group1);
        assert_eq!(removed.base_group2, added.replacement_group2);
        assert_eq!(removed.replacement_group1, added.base_group1);
        assert_eq!(removed.replacement_group2, added.base_group2);
    }
}
