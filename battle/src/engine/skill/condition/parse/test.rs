use super::*;
use crate::test_support::init_config;

#[test]
fn parses_none_conditions_and_rejects_round_start_for_active_skill() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "102");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::None(NoneMode::RoundStart)
    );
    assert!(!conditions[0].allows_active_skill());
}

#[test]
fn parses_lifecycle_enter_fight_conditions() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "5");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight
        )
    );

    let conditions = parse_conditions(config::configs::get(), "5021");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::BattleStart
        )
    );
}

#[test]
fn empty_condition_is_active_skill_safe() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "");

    assert_eq!(conditions, vec![ParsedCondition::always()]);
    assert!(conditions[0].allows_active_skill());
}

#[test]
fn parses_registered_static_buff_conditions() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "19021#101,102");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![101, 102],
        }
    );
}

#[test]
fn parses_or_groups_and_common_state_conditions() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "1#500|89#3");
    assert!(matches!(conditions[0].kind, ParsedConditionKind::Any(_)));

    let conditions = parse_conditions(config::configs::get(), "51213#31320113#2");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::BuffTypeCount {
            type_ids: vec![31320113],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 2,
        }
    );

    let conditions = parse_conditions(config::configs::get(), "585208");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::TargetIdentity {
            mode: TargetIdentityMode::TargetIsSelf,
            value: 0,
        }
    );
}

#[test]
fn parses_random_threshold_as_permille() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "552210#500");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::Random { threshold: 500 }
    );
}

#[test]
fn parses_opcode_specific_active_skill_conditions() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "34210#3,4");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::ActiveSkillEffectTag(vec![3, 4])
    );

    let conditions = parse_conditions(config::configs::get(), "500210#2");
    assert_eq!(conditions[0].kind, ParsedConditionKind::ActiveSkillType(2));

    let conditions = parse_conditions(config::configs::get(), "6208#2");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::Unsupported("UseSkillStar".into())
    );

    let conditions = parse_conditions(config::configs::get(), "662208#100,200");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::ActiveSkillId(vec![100, 200])
    );
}

#[test]
fn parses_ex_point_decrease_event_condition() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "660008#1");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::ExPointDecrease { threshold: 1 }
    );
}

#[test]
fn parses_bullet_trigger_as_an_exact_event_subscription() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "649210");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::BuffFeatureTriggered { act_id: 827 }
    );
    assert_eq!(
        conditions[0].subscriptions(),
        vec![SubscriptionKey::new(
            EventKind::BuffFeatureTriggered,
            crate::engine::skill::rule::DefinitionKey::new(649210, "TriggerBullet")
        )]
    );
}

#[test]
fn parses_opcode_specific_entity_count_conditions() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "546208#2#1");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::EntityCount {
            scope: EntityCountScope::AliveEnemiesIncludeSp,
            compare: ConditionCompare::GreaterThanOrEqual,
            count: 2,
        }
    );

    let conditions = parse_conditions(config::configs::get(), "616012#2");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::EntityCount {
            scope: EntityCountScope::AliveTeammatesNoSp,
            compare: ConditionCompare::Equal,
            count: 2,
        }
    );
}

#[test]
fn parses_per_target_career_count_condition() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "650002#5,6#1#1#1#2");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::PerTargetCareerCount {
            careers: vec![5, 6],
            threshold: 2,
        }
    );
}

#[test]
fn negated_team_career_threshold_inverts_the_comparison() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "562002#3#3!");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::TeamCareerCount {
            careers: vec![3],
            compare: ConditionCompare::LessThan,
            threshold: 3,
        }
    );
}

#[test]
fn negated_boolean_predicate_keeps_its_exact_condition_identity() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "25208!");

    assert_eq!(conditions[0].opcode, 25208);
    assert_eq!(conditions[0].type_name, "UseExSkill");
    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::Not(Box::new(ParsedConditionKind::UseExSkill))
    );
}

#[test]
fn parses_target_career_hash_args() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "16021#3#4");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::TargetCareer(vec![3, 4])
    );
}

#[test]
fn from_the_depths_injury_opcodes_keep_their_distinct_action_scopes() {
    init_config();

    let reset = parse_conditions(config::configs::get(), "629210#3");
    let persistent = parse_conditions(config::configs::get(), "630212");

    assert_eq!(
        reset[0].kind,
        ParsedConditionKind::TeammateInjuryCount {
            persistent: false,
            threshold: 3,
        }
    );
    assert_eq!(
        persistent[0].kind,
        ParsedConditionKind::TeammateInjuryCount {
            persistent: true,
            threshold: 1,
        }
    );
    assert_eq!(
        reset[0].timing(),
        ConditionTiming::Event(EventKind::SkillAction)
    );
    assert_eq!(
        persistent[0].timing(),
        ConditionTiming::Event(EventKind::AllyAction)
    );
}

#[test]
fn parses_career_and_blood_pool_max_without_collapsing_opcodes() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "16203#1&740203#50#999");

    assert_eq!(
        conditions
            .iter()
            .map(|condition| &condition.kind)
            .collect::<Vec<_>>(),
        vec![
            &ParsedConditionKind::TargetCareer(vec![1]),
            &ParsedConditionKind::BloodPoolMax { min: 50, max: 999 },
        ]
    );
}

#[test]
fn parses_round_end_heat_scale_range_with_its_config_effect() {
    init_config();

    let condition = &parse_conditions(config::configs::get(), "726304#60000#1000000#1")[0];

    assert_eq!(
        condition.kind,
        ParsedConditionKind::BloodPoolValue {
            min: 60_000,
            max: 1_000_000,
            config_effect: 1,
        }
    );
    assert_eq!(
        condition.timing(),
        ConditionTiming::Event(EventKind::RoundEndAfterSettlement)
    );

    assert_eq!(
        parse_conditions(config::configs::get(), "726304#10#99")[0].kind,
        ParsedConditionKind::BloodPoolValue {
            min: 10,
            max: 99,
            config_effect: 0,
        }
    );

    let active = &parse_conditions(config::configs::get(), "726203#120000#1000000#1")[0];
    assert_eq!(
        active.timing(),
        ConditionTiming::Event(EventKind::SkillAction)
    );
}

#[test]
fn parses_hurt_restraint_conditions_by_type() {
    init_config();

    assert_eq!(
        parse_conditions(config::configs::get(), "33201")[0].kind,
        ParsedConditionKind::HurtRestrained
    );
    assert_eq!(
        parse_conditions(config::configs::get(), "33204")[0].kind,
        ParsedConditionKind::HurtRestrained
    );
    assert_eq!(
        parse_conditions(config::configs::get(), "33209")[0].kind,
        ParsedConditionKind::HurtRestrained
    );
    assert_eq!(
        parse_conditions(config::configs::get(), "47204")[0].kind,
        ParsedConditionKind::HurtNotRestrained
    );
    assert_eq!(
        parse_conditions(config::configs::get(), "47209")[0].kind,
        ParsedConditionKind::HurtNotRestrained
    );
    assert_eq!(
        parse_conditions(config::configs::get(), "33201#1")[0].kind,
        ParsedConditionKind::Unsupported("HurtRestraint".into())
    );
}

#[test]
fn opcode_53210_checks_the_active_skill_target_count() {
    init_config();

    assert_eq!(
        parse_conditions(config::configs::get(), "53210#1")[0].kind,
        ParsedConditionKind::DamageTargetCountKind(1)
    );
}

#[test]
fn opcode_812_is_an_entity_death_subscription() {
    init_config();

    let condition = parse_conditions(config::configs::get(), "812")
        .into_iter()
        .next()
        .unwrap();

    assert_eq!(condition.kind, ParsedConditionKind::EntityDead);
    assert_eq!(
        condition.timing(),
        ConditionTiming::Event(EventKind::EntityDied)
    );
}

#[test]
fn parses_career_check_share_condition() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "508104#1");

    assert_eq!(
        conditions[0].kind,
        ParsedConditionKind::TargetSharesCasterCareer { param: 1 }
    );

    let conditions = parse_conditions(config::configs::get(), "508212#0");
    assert_eq!(
        conditions[0].subscriptions(),
        vec![SubscriptionKey::new(
            EventKind::AllyAction,
            crate::engine::skill::rule::DefinitionKey::new(508212, "CareerCheck"),
        )]
    );
}

#[test]
fn keeps_no_action_round_out_of_skill_action() {
    init_config();

    let conditions = parse_conditions(config::configs::get(), "46301");

    assert_eq!(conditions[0].kind, ParsedConditionKind::NoActionRound);
    assert_eq!(
        conditions[0].subscriptions(),
        vec![SubscriptionKey::new(
            EventKind::NoActionRound,
            crate::engine::skill::rule::DefinitionKey::new(46301, "NoActRound"),
        )]
    );
}
