use crate::engine::skill::condition::parse::{
    ConditionCompare, EntityCountScope, ParsedConditionKind, first_i32, parse_fixed, parse_i32,
};
use crate::engine::skill::target::EntityDamageType;

pub fn single_kill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::SingleKillCount {
        threshold: first_i32(args)?,
    })
}

pub fn per_kill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerKillCount {
        divisor: first_i32(args)?,
    })
}

pub fn target_count(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [count, mode] = parse_fixed(args)?;
    Some(ParsedConditionKind::EntityCount {
        scope: EntityCountScope::EnemyTargets,
        compare: if mode == 1 && count > 0 {
            ConditionCompare::GreaterThanOrEqual
        } else {
            ConditionCompare::Equal
        },
        count: if mode == 1 && count == 0 { 1 } else { count },
    })
}

pub fn enemy_alive(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::EntityCount {
        scope: EntityCountScope::AliveEnemies,
        compare: if args.get(1).and_then(|arg| parse_i32(arg)) == Some(1) {
            ConditionCompare::GreaterThanOrEqual
        } else {
            ConditionCompare::Equal
        },
        count: first_i32(args)?,
    })
}

pub fn enemies_with_special_at_least(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    entity_count(
        EntityCountScope::AliveEnemiesIncludeSp,
        ConditionCompare::GreaterThanOrEqual,
        args,
    )
}

pub fn enemies_with_special_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    entity_count(
        EntityCountScope::AliveEnemiesIncludeSp,
        ConditionCompare::Equal,
        args,
    )
}

pub fn enemies_with_special_at_most(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    entity_count(
        EntityCountScope::AliveEnemiesIncludeSp,
        ConditionCompare::LessThanOrEqual,
        args,
    )
}

pub fn teammate_alive(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::EntityCount {
        scope: EntityCountScope::AliveTeammates,
        compare: ConditionCompare::GreaterThanOrEqual,
        count: 1,
    })
}

pub fn teammates_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    entity_count(
        EntityCountScope::AliveOtherTeammates,
        ConditionCompare::Equal,
        args,
    )
}

pub fn teammates_up_to_three(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [_count] = args else {
        return None;
    };
    let condition = entity_count(
        EntityCountScope::AliveOtherTeammates,
        ConditionCompare::Equal,
        args,
    )?;
    matches!(
        &condition,
        ParsedConditionKind::EntityCount { count: 0..=3, .. }
    )
    .then_some(condition)
}

pub fn other_ally_damage_type(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [raw_damage_type, raw_max_count] = args else {
        return None;
    };
    let damage_type = EntityDamageType::from_wire(raw_damage_type.parse().ok()?);
    if damage_type == EntityDamageType::Unknown {
        return None;
    }
    Some(ParsedConditionKind::OtherAllyDamageTypeCount {
        damage_type,
        max_count: raw_max_count.parse().ok()?,
    })
}

pub fn teammates_without_special(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    entity_count(
        EntityCountScope::AliveTeammatesNoSp,
        ConditionCompare::Equal,
        args,
    )
}

pub fn alive_allied_model_at_least(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    let [team, model_id, minimum] = parse_fixed(args)?;
    (team == 1 && model_id > 0 && minimum > 0)
        .then_some(ParsedConditionKind::AliveAlliedModelCount { model_id, minimum })
}

pub fn summoned_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    summoned_count(args, ConditionCompare::Equal)
}

pub fn mature_summoned_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let condition = summoned_count(args, ConditionCompare::Equal)?;
    matches!(
        &condition,
        ParsedConditionKind::SummonedCount {
            summoned_id,
            required_level: 4,
            count: 1 | 2,
            ..
        } if *summoned_id > 0
    )
    .then_some(condition)
}

pub fn summoned_at_least(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    summoned_count(args, ConditionCompare::GreaterThanOrEqual)
}

pub fn group_summoned_at_least(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    group_summoned_count(args, ConditionCompare::GreaterThanOrEqual)
}

pub fn group_summoned_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    group_summoned_count(args, ConditionCompare::Equal)
}

pub fn teammate_dead(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::TeammateDead)
}

pub fn enemy_dead(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::EnemyDead)
}

fn summoned_count(args: &[String], compare: ConditionCompare) -> Option<ParsedConditionKind> {
    let [summoned_id, required_level, count] = parse_fixed(args)?;
    Some(ParsedConditionKind::SummonedCount {
        summoned_id,
        required_level,
        compare,
        count,
    })
}

fn group_summoned_count(args: &[String], compare: ConditionCompare) -> Option<ParsedConditionKind> {
    let [owner_model_id, required_level, count] = parse_fixed(args)?;
    Some(ParsedConditionKind::GroupSummonedCount {
        owner_model_id,
        required_level,
        compare,
        count,
    })
}

fn entity_count(
    scope: EntityCountScope,
    compare: ConditionCompare,
    args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::EntityCount {
        scope,
        compare,
        count: first_i32(args)?,
    })
}

#[cfg(test)]
mod tests;
