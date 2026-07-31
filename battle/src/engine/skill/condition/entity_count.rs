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

pub fn summoned_equal(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    summoned_count(args, ConditionCompare::Equal)
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
mod tests {
    use super::*;

    #[test]
    fn parses_event_routed_counts_without_owning_their_route() {
        assert_eq!(
            single_kill(11210, "SingleKillNum", &["1".into()]),
            Some(ParsedConditionKind::SingleKillCount { threshold: 1 })
        );
        assert_eq!(
            per_kill(99210, "PerKillNum", &["1".into()]),
            Some(ParsedConditionKind::PerKillCount { divisor: 1 })
        );
        assert!(matches!(
            teammate_alive(24102, "TeammateAlive", &[]),
            Some(ParsedConditionKind::EntityCount { .. })
        ));
        assert!(matches!(
            group_summoned_equal(
                524302,
                "GroupSummonedNumEqual",
                &["1".into(), "0".into(), "1".into()]
            ),
            Some(ParsedConditionKind::GroupSummonedCount { .. })
        ));
    }

    #[test]
    fn target_count_keeps_its_after_hit_route() {
        let definition = super::super::registry::find_key(717210, "TargetCount").unwrap();

        assert_eq!(
            definition.role,
            super::super::registry::ConditionRole::Trigger {
                event: crate::engine::event::kind::EventKind::SkillAction,
                phase: Some(crate::engine::skill::action::SkillPhase::AfterHit),
            }
        );
    }

    #[test]
    fn ezio_enemy_count_keeps_its_skill_action_route() {
        let definition = super::super::registry::find_key(1011201, "EnemyAliveNum").unwrap();

        assert_eq!(
            definition.role,
            super::super::registry::ConditionRole::Trigger {
                event: crate::engine::event::kind::EventKind::SkillAction,
                phase: None,
            }
        );
        assert!(matches!(
            enemy_alive(1011201, "EnemyAliveNum", &["1".into()]),
            Some(ParsedConditionKind::EntityCount {
                scope: EntityCountScope::AliveEnemies,
                compare: ConditionCompare::Equal,
                count: 1,
            })
        ));
    }

    #[test]
    fn summon_and_death_rows_keep_exact_scopes() {
        assert_eq!(
            summoned_at_least(
                520203,
                "SummonedNumMoreThan",
                &["150011".into(), "0".into(), "1".into()]
            ),
            Some(ParsedConditionKind::SummonedCount {
                summoned_id: 150011,
                required_level: 0,
                compare: ConditionCompare::GreaterThanOrEqual,
                count: 1,
            })
        );
        assert_eq!(
            group_summoned_equal(
                524302,
                "GroupSummonedNumEqual",
                &["3074".into(), "0".into(), "2".into()]
            ),
            Some(ParsedConditionKind::GroupSummonedCount {
                owner_model_id: 3074,
                required_level: 0,
                compare: ConditionCompare::Equal,
                count: 2,
            })
        );
        assert_eq!(
            teammate_dead(17, "TeammateDead", &[]),
            Some(ParsedConditionKind::TeammateDead)
        );
        assert_eq!(
            enemy_dead(86, "EnemyDead", &[]),
            Some(ParsedConditionKind::EnemyDead)
        );
    }
}
