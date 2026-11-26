use crate::engine::{
    manager::BattleManagers,
    skill::condition::{
        ParsedCondition,
        parse::{BuffAddedScope, ConditionCompare, ParsedConditionKind},
    },
};

use super::parse::parse_fixed;

pub fn team_enemy_layers(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [type_id, threshold] = parse_fixed(args)?;
    (type_id > 0 && threshold > 0)
        .then_some(ParsedConditionKind::AccTeamEnemyBuffLayers { type_id, threshold })
}

pub fn team_enemy_layer_repeats(
    type_id: i32,
    threshold: i32,
    source_team: Option<i32>,
    context: crate::engine::skill::target::TargetContext,
) -> i32 {
    context
        .team_buff_layers
        .filter(|settled| Some(settled.team) == source_team && settled.type_id == type_id)
        .map_or(0, |settled| {
            settled.after / threshold - settled.before / threshold
        })
}

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

pub fn added_type_count_repeats(
    condition: &ParsedCondition,
    managers: &BattleManagers,
    context: crate::engine::skill::target::TargetContext,
) -> i32 {
    if context.added_buff_amount <= 0 {
        return 0;
    }
    let (total, threshold) = match &condition.kind {
        ParsedConditionKind::AccBuffTypeAddedCount {
            type_ids,
            threshold,
        } if type_ids.contains(&context.added_buff_type_id) => (
            managers
                .buff
                .added_type_count_for_owner(context.added_buff_target_uid, type_ids),
            *threshold,
        ),
        ParsedConditionKind::AccCommittedBuffTypeAddedCount { type_id, threshold }
            if *type_id == context.added_buff_type_id =>
        {
            (
                context.added_buff_type_total.unwrap_or_default(),
                *threshold,
            )
        }
        _ => return 0,
    };
    let threshold = threshold.max(1);
    let previous = (total - context.added_buff_amount).max(0);
    total / threshold - previous / threshold
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

pub fn per_bullet(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [divisor, max_count] = raw_args else {
        return None;
    };
    let divisor = divisor.parse().ok()?;
    let max_count = max_count.parse().ok()?;
    (divisor > 0 && max_count > 0).then_some(ParsedConditionKind::PerBullet { divisor, max_count })
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
    let [layers_per_repeat, max_repeats, type_ids] = raw_args else {
        return None;
    };
    let layers_per_repeat = layers_per_repeat.parse().ok()?;
    let max_repeats = max_repeats.parse().ok()?;
    if layers_per_repeat <= 0 || max_repeats <= 0 {
        return None;
    }
    Some(ParsedConditionKind::PerBuffTypeLayer {
        layers_per_repeat,
        max_repeats,
        type_ids: parse_buff_ids(std::slice::from_ref(type_ids))?,
    })
}

pub fn buff_added(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let first_ids = || parse_buff_ids(raw_args.get(..1)?);
    Some(ParsedConditionKind::BuffAdded(first_ids()?))
}

pub fn buff_removed(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffRemoved(parse_buff_ids(raw_args)?))
}

pub fn buff_type_added(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffTypeAdded(parse_buff_ids(
        raw_args,
    )?))
}

pub fn buff_expired(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffExpired(parse_buff_ids(raw_args)?))
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

pub fn status_absent(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let statuses = parse_buff_ids(raw_args)?
        .into_iter()
        .map(crate::engine::manager::buff::BuffStatus::from_id)
        .collect::<Vec<_>>();
    statuses
        .iter()
        .all(|status| *status != crate::engine::manager::buff::BuffStatus::Unknown)
        .then_some(ParsedConditionKind::PrimaryStatusAbsent(statuses))
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

pub fn each_buff_id_layer(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let buff_ids = parse_buff_ids(raw_args)?;
    (buff_ids.len() == 1).then_some(ParsedConditionKind::BuffIdCount {
        buff_ids,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: 1,
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

pub fn player_buff_presence(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [owner, present, buff_id] = parse_fixed(raw_args)?;
    (owner == 2).then_some(ParsedConditionKind::PlayerBuffPresence {
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

pub fn owner_added_type_count(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AccBuffTypeAddedCount {
        type_ids: parse_buff_ids(raw_args.get(..1)?)?,
        threshold: raw_args.get(1)?.parse().ok()?,
    })
}

pub fn owner_added_single_type(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [type_id, threshold] = parse_fixed(raw_args)?;
    (type_id > 0 && threshold > 0)
        .then_some(ParsedConditionKind::AccCommittedBuffTypeAddedCount { type_id, threshold })
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

pub fn all_targets_buff_type_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AllTargetBuffTypeCount {
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

pub fn team_buff_type_layer_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BuffTypeCount {
        threshold: raw_args.first()?.parse().ok()?,
        type_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::GreaterThanOrEqual,
    })
}

pub fn death_team_buff_type_layer(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [threshold, type_id] = parse_fixed(raw_args)?;
    (threshold == 1 && type_id > 0).then_some(ParsedConditionKind::BuffTypeCount {
        threshold,
        type_ids: vec![type_id],
        compare: ConditionCompare::GreaterThanOrEqual,
    })
}

pub fn death_team_buff_type_absent(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [threshold, type_id] = parse_fixed(raw_args)?;
    (threshold == 0 && type_id > 0).then_some(ParsedConditionKind::BuffTypeCount {
        threshold,
        type_ids: vec![type_id],
        compare: ConditionCompare::LessThanOrEqual,
    })
}

pub fn round_team_buff_type_at_most(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [threshold, type_id] = parse_fixed(raw_args)?;
    (threshold >= 0 && type_id > 0).then_some(ParsedConditionKind::BuffTypeCount {
        threshold,
        type_ids: vec![type_id],
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

pub fn visible_buff_status_types_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::VisibleBuffStatusTypeCount {
        status_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: raw_args.first()?.parse().ok()?,
    })
}

pub fn visible_stacked_status_types_at_least(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::VisibleStackedStatusTypeCount {
        status_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: raw_args.first()?.parse().ok()?,
    })
}

pub fn visible_status_threshold(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    let [raw_threshold, raw_status_ids] = raw_args else {
        return None;
    };
    let threshold = raw_threshold.parse().ok()?;
    let status_ids = parse_buff_ids(std::slice::from_ref(raw_status_ids))?;
    ((1..=3).contains(&threshold) && status_ids.iter().all(|status_id| *status_id > 0)).then_some(
        ParsedConditionKind::VisibleStackedStatusTypeCount {
            status_ids,
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold,
        },
    )
}

pub fn visible_stacked_status_below(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::VisibleStackedStatusTypeCount {
        status_ids: parse_buff_ids(raw_args.get(1..2)?)?,
        compare: ConditionCompare::LessThan,
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

pub fn rejected_buff_id_or_type(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    match raw_args {
        [value] => value
            .parse::<i32>()
            .ok()
            .filter(|value| *value > 0)
            .map(ParsedConditionKind::RejectedBuffIdOrType),
        _ => None,
    }
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

pub fn single_buff_absent(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [raw_id] = raw_args else {
        return None;
    };
    let buff_id = raw_id.parse::<i32>().ok().filter(|buff_id| *buff_id > 0)?;
    Some(ParsedConditionKind::BuffId {
        mode: BuffConditionMode::Absent,
        buff_ids: vec![buff_id],
    })
}

pub fn positive_buff_absent(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let condition = buff_presence(raw_args, BuffConditionMode::Absent)?;
    matches!(
        &condition,
        ParsedConditionKind::BuffId { buff_ids, .. }
            if buff_ids.iter().all(|buff_id| *buff_id > 0)
    )
    .then_some(condition)
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
            ParsedConditionKind::PerBuffTypeLayer {
                type_ids,
                layers_per_repeat,
                max_repeats,
            } => {
                let count = holder_layers(type_ids, targets, managers).unwrap_or_default();
                Some((count / layers_per_repeat).min(*max_repeats))
            }
            _ => None,
        })
        .min()
        .unwrap_or(1)
}

pub(super) fn holder_layers(
    type_ids: &[i32],
    targets: &[i64],
    managers: &BattleManagers,
) -> Option<i32> {
    let [holder] = targets else {
        return None;
    };
    Some(
        type_ids
            .iter()
            .map(|id| managers.buff.buff_type_amount(*holder, *id))
            .sum(),
    )
}

pub(crate) fn reads_one_holder(conditions: &[ParsedCondition]) -> bool {
    fn reads(kind: &ParsedConditionKind) -> bool {
        match kind {
            ParsedConditionKind::PerBuffTypeLayer { .. } => true,
            ParsedConditionKind::Not(condition) => reads(condition),
            ParsedConditionKind::Any(branches) => {
                branches.iter().any(|branch| reads_one_holder(branch))
            }
            _ => false,
        }
    }
    conditions.iter().any(|condition| reads(&condition.kind))
}

pub(crate) fn reads_hit_recipient_layers(conditions: &[ParsedCondition]) -> bool {
    let [condition] = conditions else {
        return false;
    };
    matches!(condition.kind, ParsedConditionKind::PerBuffTypeLayer { .. })
        && super::registry::find_key(condition.opcode, &condition.type_name)
            .is_some_and(|definition| definition.role == super::registry::ConditionRole::Predicate)
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
mod tests;
