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

#[test]
fn alive_monster_count_requires_the_allied_team_shape() {
    assert_eq!(
        alive_allied_model_at_least(
            74210,
            "MonsterAliveNumMoreThan",
            &["1".into(), "103370112".into(), "1".into()]
        ),
        Some(ParsedConditionKind::AliveAlliedModelCount {
            model_id: 103370112,
            minimum: 1,
        })
    );
    assert!(
        alive_allied_model_at_least(
            74210,
            "MonsterAliveNumMoreThan",
            &["2".into(), "103370112".into(), "1".into()]
        )
        .is_none()
    );
}
