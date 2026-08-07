use crate::engine::{
    manager::BattleManagers,
    skill::condition::{
        ParsedCondition,
        parse::{BuffAddedScope, ConditionCompare, ParsedConditionKind},
    },
};

use super::parse::parse_fixed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffConditionMode {
    Present,
    PresentAndConsume,
    Absent,
    ExactPresent,
    ExactAbsent,
}

pub fn added_count_repeats(
    condition: &ParsedCondition,
    source_uid: i64,
    managers: &BattleManagers,
    context: crate::engine::skill::target::TargetContext,
) -> i32 {
    let ParsedConditionKind::AccBuffAddedCount {
        buff_ids,
        threshold,
        scope,
    } = &condition.kind
    else {
        return 0;
    };
    if context.added_buff_amount <= 0 || !buff_ids.contains(&context.added_buff_id) {
        return 0;
    }
    let total = match scope {
        BuffAddedScope::Owner => managers
            .buff
            .added_count_for_owner(context.added_buff_target_uid, buff_ids),
        BuffAddedScope::Team => managers
            .buff
            .team_type(source_uid)
            .map(|team| managers.buff.added_count_for_team(team, buff_ids))
            .unwrap_or_default(),
    };
    let threshold = (*threshold).max(1);
    let previous = (total - context.added_buff_amount).max(0);
    let repeats = total / threshold - previous / threshold;
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Skill) {
        eprintln!(
            "accumulated buff condition key={}/{} source={} target={} buff={} delta={} previous={} total={} threshold={} repeats={}",
            condition.opcode,
            condition.type_name,
            source_uid,
            context.added_buff_target_uid,
            context.added_buff_id,
            context.added_buff_amount,
            previous,
            total,
            threshold,
            repeats,
        );
    }
    repeats
}

pub fn buff_group(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffGroup(parse_buff_ids(raw_args)?))
}

pub fn per_buff_group_count(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [group_id] = raw_args else {
        return None;
    };
    let group_id = group_id.parse().ok()?;
    (group_id > 0).then_some(ParsedConditionKind::PerBuffGroupCount { group_id })
}

pub fn no_buff_group(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::NoBuffGroup(parse_buff_ids(raw_args)?))
}

pub fn from_and_to_buff(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::FromBuffAndToBuff {
        from_buff_id: raw_args.first()?.parse().ok()?,
        to_buff_id: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn self_buff_type_target_buff_types(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::SelfBuffTypeTargetBuffTypes {
        self_type_id: raw_args.first()?.parse().ok()?,
        target_type_ids: parse_buff_ids(raw_args.get(1..2)?)?,
    })
}

pub fn per_type_layer(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerBuffTypeLayer {
        min: raw_args.first()?.parse().ok()?,
        max: raw_args.get(1)?.parse().ok()?,
        type_ids: parse_buff_ids(raw_args.get(2..3)?)?,
    })
}

pub fn buff_added(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let first_ids = || parse_buff_ids(raw_args.get(..1)?);
    Some(ParsedConditionKind::BuffAdded(first_ids()?))
}

pub fn buff_removed(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffRemoved(parse_buff_ids(raw_args)?))
}

pub fn any_status_present(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    status_count(parse_buff_ids(raw_args)?, 1)
}

pub fn first_status_present(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    status_count(parse_buff_ids(raw_args.get(..1)?)?, 1)
}

pub fn first_status_absent(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffStatusCount {
        status_ids: parse_buff_ids(raw_args.get(..1)?)?,
        compare: ConditionCompare::Equal,
        threshold: 0,
    })
}

fn status_count(status_ids: Vec<i32>, threshold: i32) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffStatusCount {
        status_ids,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold,
    })
}

pub fn per_buff_id_count(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffIdCount {
        buff_ids: parse_buff_ids(raw_args.get(..1)?)?,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: raw_args
            .get(1)
            .and_then(|arg| arg.parse().ok())
            .unwrap_or(1),
    })
}

pub fn team_added_count(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AccBuffAddedCount {
        buff_ids: parse_buff_ids(raw_args.get(..1)?)?,
        threshold: raw_args
            .get(1)
            .and_then(|arg| arg.parse().ok())
            .unwrap_or(1),
        scope: BuffAddedScope::Team,
    })
}

pub fn per_buff_id(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffIdCount {
        buff_ids: parse_buff_ids(raw_args)?,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: 1,
    })
}

pub fn buff_id_at_least(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffIdThreshold {
        buff_ids: parse_buff_ids(raw_args.get(..1)?)?,
        threshold: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn team_buff_presence(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [team, present, buff_id] = parse_fixed(raw_args)?;
    Some(ParsedConditionKind::TeamBuffPresence {
        team,
        present: present != 0,
        buff_id,
    })
}

pub fn owner_added_count(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AccBuffAddedCount {
        buff_ids: parse_buff_ids(raw_args.get(..1)?)?,
        threshold: raw_args.get(1)?.parse().ok()?,
        scope: BuffAddedScope::Owner,
    })
}

pub fn buff_type_at_least(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    buff_type_count(raw_args, ConditionCompare::GreaterThanOrEqual)
}

pub fn any_target_buff_type_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AnyTargetBuffTypeCount {
        type_ids: parse_buff_ids(raw_args.get(..1)?)?,
        threshold: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn buff_type_pair_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    if raw_args.len() != 2 {
        return None;
    }
    buff_type_count(raw_args, ConditionCompare::GreaterThanOrEqual)
}

pub fn positive_buff_type_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [type_id, threshold] = parse_fixed(raw_args)?;
    (type_id > 0 && threshold > 0).then_some(ParsedConditionKind::BuffTypeCount {
        type_ids: vec![type_id],
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold,
    })
}

pub fn buff_type_at_most(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    buff_type_count(raw_args, ConditionCompare::LessThanOrEqual)
}

pub fn team_buff_type_layer_at_most(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffTypeCount {
        threshold: raw_args.first()?.parse().ok()?,
        type_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::LessThanOrEqual,
    })
}

fn buff_type_count(raw_args: &[String], compare: ConditionCompare) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffTypeCount {
        type_ids: parse_buff_ids(raw_args.get(..1)?)?,
        compare,
        threshold: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn buff_status_at_least(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    status_count(
        parse_buff_ids(raw_args.get(1..2)?)?,
        raw_args.first()?.parse().ok()?,
    )
}

pub fn buff_status_at_most(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffStatusCount {
        status_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::LessThanOrEqual,
        threshold: raw_args.first()?.parse().ok()?,
    })
}

pub fn per_team_status_type_count(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [divisor, max_count, status_ids] = raw_args else {
        return None;
    };
    let divisor = divisor.parse().ok()?;
    let max_count = max_count.parse().ok()?;
    let status_ids = parse_buff_ids(std::slice::from_ref(status_ids))?;
    if divisor <= 0 || max_count <= 0 || status_ids.iter().any(|status| *status <= 0) {
        return None;
    }
    Some(ParsedConditionKind::PerTeamBuffStatusTypeCount {
        divisor,
        max_count,
        status_ids,
    })
}

pub fn per_distinct_status_type_count(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerTeamBuffStatusTypeCount {
        status_ids: parse_buff_ids(raw_args)?,
        divisor: 1,
        max_count: i32::MAX,
    })
}

pub fn enemy_highest_buff_type_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::EnemyHighestBuffTypeCount {
        type_id: raw_args.first()?.parse().ok()?,
        threshold: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn burn_overflow(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::BurnOverflow)
}

pub fn master_halo(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::MasterHalo)
}

pub fn buff_present(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    buff_presence(raw_args, BuffConditionMode::Present)
}

pub fn exact_buff_present(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    buff_presence(raw_args, BuffConditionMode::ExactPresent)
}

pub fn buff_present_and_consume(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    buff_presence(raw_args, BuffConditionMode::PresentAndConsume)
}

pub fn buff_absent(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    buff_presence(raw_args, BuffConditionMode::Absent)
}

fn buff_presence(raw_args: &[String], mode: BuffConditionMode) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffId {
        mode,
        buff_ids: parse_buff_ids(raw_args)?,
    })
}

pub fn per_type_layer_count(
    conditions: &[ParsedCondition],
    targets: &[i64],
    managers: &BattleManagers,
) -> i32 {
    conditions
        .iter()
        .filter_map(|condition| match &condition.kind {
            ParsedConditionKind::PerBuffTypeLayer { type_ids, min, max } => {
                let count = targets
                    .iter()
                    .map(|uid| {
                        type_ids
                            .iter()
                            .map(|type_id| managers.buff.buff_type_amount(*uid, *type_id))
                            .sum::<i32>()
                    })
                    .sum::<i32>();
                let min = (*min).max(0);
                Some(if count < min {
                    0
                } else {
                    count.min((*max).max(min))
                })
            }
            _ => None,
        })
        .min()
        .unwrap_or(1)
}

fn parse_buff_ids(raw_args: &[String]) -> Option<Vec<i32>> {
    let mut ids = Vec::new();
    for raw in raw_args {
        for part in raw
            .trim()
            .trim_end_matches('!')
            .trim_end_matches('！')
            .split([',', '，'])
        {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            ids.push(part.parse().ok()?);
        }
    }

    (!ids.is_empty()).then_some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn parses_static_buff_presence_lists() {
        assert_eq!(
            buff_present(19021, "HasBuffId", &["1,2".to_owned()]),
            Some(ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![1, 2],
            })
        );
        assert_eq!(
            buff_absent(57304, "NoBuffId", &["1".to_owned()]),
            Some(ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Absent,
                buff_ids: vec![1],
            })
        );
        assert_eq!(
            buff_present_and_consume(19208, "HasBuffId", &["8178".to_owned()]),
            Some(ParsedConditionKind::BuffId {
                mode: BuffConditionMode::PresentAndConsume,
                buff_ids: vec![8178],
            })
        );
    }

    #[test]
    fn buff_conditions_keep_status_and_added_id_semantics() {
        assert_eq!(
            buff_added(10, "BuffIdAdd", &["31070111".into()]),
            Some(ParsedConditionKind::BuffAdded(vec![31070111]))
        );
        assert_eq!(
            first_status_present(18203, "HasBuff", &["14".into()]),
            Some(ParsedConditionKind::BuffStatusCount {
                status_ids: vec![14],
                compare: ConditionCompare::GreaterThanOrEqual,
                threshold: 1,
            })
        );
        assert_eq!(
            buff_removed(49, "BuffIdDel", &["31070111".into()]),
            Some(ParsedConditionKind::BuffRemoved(vec![31070111]))
        );
    }

    #[test]
    fn skill_action_status_condition_keeps_every_configured_category() {
        assert_eq!(
            any_status_present(18202, "HasBuff", &["2".into(), "4".into(), "6".into()],),
            Some(ParsedConditionKind::BuffStatusCount {
                status_ids: vec![2, 4, 6],
                compare: ConditionCompare::GreaterThanOrEqual,
                threshold: 1,
            })
        );
        assert_eq!(
            first_status_present(18203, "HasBuff", &["1".into(), "1".into(), "1006".into()],),
            Some(ParsedConditionKind::BuffStatusCount {
                status_ids: vec![1],
                compare: ConditionCompare::GreaterThanOrEqual,
                threshold: 1,
            })
        );
    }

    #[test]
    fn buff_group_conditions_keep_their_exact_opcode_and_polarity() {
        assert_eq!(
            buff_group(77203, "HasBuffGroup", &["5".into()]),
            Some(ParsedConditionKind::BuffGroup(vec![5]))
        );
        assert_eq!(
            buff_group(77208, "HasBuffGroup", &["5".into()]),
            Some(ParsedConditionKind::BuffGroup(vec![5]))
        );
        assert_eq!(
            no_buff_group(78208, "NoBuffGroup", &["5".into()]),
            Some(ParsedConditionKind::NoBuffGroup(vec![5]))
        );
    }

    #[test]
    fn from_and_to_buff_keeps_both_configured_identities() {
        assert_eq!(
            from_and_to_buff(
                1007204,
                "FromBuffAndToBuff",
                &["229101".into(), "229102".into()],
            ),
            Some(ParsedConditionKind::FromBuffAndToBuff {
                from_buff_id: 229101,
                to_buff_id: 229102,
            })
        );
    }

    #[test]
    fn per_buff_type_layer_returns_the_capped_stack_count() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::default();
        managers.buff.seed(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        buff_id: Some(31340002),
                        layer: Some(7),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let condition = ParsedCondition {
            opcode: 518203,
            type_name: String::new(),
            kind: ParsedConditionKind::PerBuffTypeLayer {
                type_ids: vec![31340002],
                min: 1,
                max: 20,
            },
            raw_args: Vec::new(),
        };

        assert_eq!(
            per_type_layer_count(std::slice::from_ref(&condition), &[10], &managers),
            7
        );
        let below_minimum = ParsedCondition {
            kind: ParsedConditionKind::PerBuffTypeLayer {
                type_ids: vec![31340002],
                min: 8,
                max: 20,
            },
            ..condition
        };
        assert_eq!(per_type_layer_count(&[below_minimum], &[10], &managers), 0);
    }
}
