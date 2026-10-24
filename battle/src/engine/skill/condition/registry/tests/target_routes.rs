use super::*;

#[test]
fn other_ally_extra_action_keeps_its_exact_route() {
    assert_eq!(
        parse(403212, "SkillExtraType", &["1".into()]),
        Some(ParsedConditionKind::ExtraAction {
            mode: super::super::super::extra::ExtraActionConditionMode::OtherAllyAction,
            kinds: vec![1],
        })
    );
    assert_eq!(
        find_key(403212, "SkillExtraType").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert!(find_key(403212, "UseSkill").is_none());
}

#[test]
fn other_ally_action_kind_keeps_its_exact_route() {
    assert_eq!(
        parse(626212, "ActionSkillExtraType", &["1,2,3".into()]),
        Some(ParsedConditionKind::ExtraAction {
            mode: super::super::super::extra::ExtraActionConditionMode::OtherAllyAction,
            kinds: vec![1, 2, 3],
        })
    );
    assert_eq!(
        find_key(626212, "ActionSkillExtraType").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert!(find_key(626212, "SkillExtraType").is_none());
}

#[test]
fn bound_ally_buff_types_keep_their_exact_route() {
    assert_eq!(
        parse(
            656212,
            "SelfBuffTypeTargetBuffTypes",
            &["31000201".into(), "31000171,31000181".into()],
        ),
        Some(ParsedConditionKind::SelfBuffTypeTargetBuffTypes {
            self_type_id: 31000201,
            target_type_ids: vec![31000171, 31000181],
        })
    );
    assert_eq!(
        find_key(656212, "SelfBuffTypeTargetBuffTypes").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert!(find_key(656212, "FromBuffAndToBuff").is_none());
}

#[test]
fn owner_incantation_rank_keeps_its_exact_route() {
    assert_eq!(
        parse(659212, "UseSkill", &["1".into()]),
        Some(ParsedConditionKind::UseSkillRank(vec![1]))
    );
    assert_eq!(
        find_key(659212, "UseSkill").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert!(find_key(659212, "ActiveUseSkill").is_none());
}

#[test]
fn eureka_threshold_keeps_same_and_opposing_action_routes_distinct() {
    let expected = ParsedConditionKind::PowerCompare {
        compare_code: 1,
        power_id: 1,
        threshold: 5,
    };
    for opcode in [180212999, 180213999] {
        assert_eq!(
            parse(
                opcode,
                "PowerCompare",
                &["1".into(), "1".into(), "5".into()]
            ),
            Some(expected.clone())
        );
        assert_eq!(
            find_key(opcode, "PowerCompare").map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::AllyAction,
                phase: None,
            })
        );
    }
    assert_eq!(
        find_key(180212999, "PowerCompare").map(|definition| definition.skill_action_observer),
        Some(SkillActionObserver::Team)
    );
    assert_eq!(
        find_key(180213999, "PowerCompare").map(|definition| definition.skill_action_observer),
        Some(SkillActionObserver::OpposingTeam)
    );
    assert!(find_key(180212999, "PowerCompareOther").is_none());
}

#[test]
fn enter_fight_team_career_threshold_keeps_its_exact_key() {
    assert_eq!(
        parse(562002, "CareerGroupHeroCountGE", &["3".into(), "3".into()]),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![3],
            compare: super::super::super::parse::ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert_eq!(
        find_key(562002, "CareerGroupHeroCountGE").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::EnterFight,
            priority: 0,
        })
    );
    assert_eq!(
        parse(
            562101,
            "CareerGroupHeroCountGE",
            &["3,5,6".into(), "3".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![3, 5, 6],
            compare: super::super::super::parse::ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert_eq!(
        find_key(562101, "CareerGroupHeroCountGE").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
    assert_eq!(
        parse(
            560100,
            "CareerGroupHeroCountLE",
            &["3,5,6".into(), "2".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![3, 5, 6],
            compare: super::super::super::parse::ConditionCompare::LessThanOrEqual,
            threshold: 2,
        })
    );
    assert_eq!(
        find_key(560100, "CareerGroupHeroCountLE").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 100,
        })
    );
    assert!(find_key(562003, "CareerGroupHeroCountGE").is_none());
}

#[test]
fn enter_fight_team_career_multiplier_keeps_its_exact_key() {
    assert_eq!(
        parse(
            517010,
            "PerHasTargetCareer",
            &["2".into(), "1".into(), "1".into()],
        ),
        Some(ParsedConditionKind::PerTeamCareerCount {
            career: 2,
            divisor: 1,
            opposing_team: false,
        })
    );
    assert_eq!(
        find_key(517010, "PerHasTargetCareer").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::OpeningSync,
            priority: 0,
        })
    );
    assert!(find_key(517, "PerHasTargetCareer").is_none());
}

#[test]
fn exact_target_team_career_count_keeps_its_round_start_route() {
    assert_eq!(
        parse(
            516101,
            "HasTargetCareerNum",
            &["4".into(), "1".into(), "3".into(), "1".into()],
        ),
        Some(ParsedConditionKind::TeamCareerCount {
            careers: vec![4],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert_eq!(
        find_key(516101, "HasTargetCareerNum").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
    for opcode in [516, 516010, 516203, 516208, 516212] {
        assert!(find_key(opcode, "HasTargetCareerNum").is_none());
    }
}

#[test]
fn enter_fight_reactivates_for_the_entered_or_transformed_owner() {
    assert_eq!(
        find_key(5, "EnterFight").unwrap().reactivation_events,
        &[EventKind::EntityEntered, EventKind::EntityTransformed]
    );
    assert!(
        find_key(595002, "TargetIncludeHero")
            .unwrap()
            .reactivation_events
            .is_empty()
    );
}

#[test]
fn exact_target_model_round_start_key_keeps_its_own_route() {
    assert_eq!(
        find_key(595101, "TargetIncludeHero").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
    assert!(matches!(
        parse(595101, "TargetIncludeHero", &["3102".into()]),
        Some(ParsedConditionKind::TargetIdentity {
            mode: super::super::super::parse::TargetIdentityMode::TargetModelId,
            value: 3102,
        })
    ));
    assert!(find_key(595203, "TargetIncludeHero").is_none());
    assert_eq!(
        find_key(595210, "TargetIncludeHero").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert!(matches!(
        parse(595210, "TargetIncludeHero", &["3102".into()]),
        Some(ParsedConditionKind::TargetIdentity {
            mode: super::super::super::parse::TargetIdentityMode::ActiveSkillSourceModelId,
            value: 3102,
        })
    ));
}

#[test]
fn target_career_selects_matching_behavior_targets() {
    let condition = ParsedCondition {
        opcode: 16210,
        type_name: "TargetCareer".into(),
        kind: ParsedConditionKind::TargetCareer(vec![3]),
        raw_args: vec!["3".into()],
    };

    assert_eq!(
        behavior_target_order(&[condition]),
        Some(BehaviorTargetOrder::EntityIdentityMap)
    );
    assert_eq!(
        find_key(16210, "TargetCareer").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    for opcode in [16002, 16210] {
        assert_eq!(
            behavior_target_order(&[ParsedCondition {
                opcode,
                type_name: "TargetCareer".into(),
                kind: ParsedConditionKind::TargetCareer(vec![3]),
                raw_args: vec!["3".into()],
            }]),
            Some(BehaviorTargetOrder::EntityIdentityMap)
        );
    }
    assert!(
        !find_key(16203, "TargetCareer")
            .is_some_and(|definition| definition.filters_behavior_targets)
    );
}

#[test]
fn round_start_buff_gates_keep_their_exact_registered_keys() {
    for (opcode, priority) in [(19100, 100), (19101, 101), (19102, 102)] {
        assert_eq!(
            find_key(opcode, "HasBuffId").map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::RoundStartCondition,
                priority,
            })
        );
        assert_eq!(
            find_key(opcode, "HasBuffId").map(|definition| definition.dependencies),
            Some(&[][..])
        );
        assert_eq!(
            parse(opcode, "HasBuffId", &["109360006".into()]),
            Some(ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![109360006],
            })
        );
        assert!(
            find_key(opcode, "HasBuffId")
                .unwrap()
                .filters_behavior_targets
        );
    }
}

#[test]
fn round_start_card_buff_gate_is_a_pure_exact_predicate() {
    let definition = find_key(19106, "HasBuffId").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(19106, "HasBuffId", &["30870131".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![30870131],
        })
    );
    assert!(parse(19106, "HasBuffId", &[]).is_none());
    assert!(find_key(19106, "NoBuffId").is_none());
}

#[test]
fn mirror_rule_buff_gates_keep_their_exact_phases() {
    assert_eq!(
        parse(57100, "NoBuffId", &["11790011".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![11790011],
        })
    );
    assert_eq!(
        find_key(57100, "NoBuffId").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        find_key(57100, "NoBuffId").map(|definition| definition.dependencies),
        Some(&[EventKind::RoundStart][..])
    );

    assert_eq!(
        find_key(19209, "HasBuffId").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        find_key(19209, "HasBuffId").map(|definition| definition.dependencies),
        Some(&[EventKind::TargetAttacked][..])
    );

    for (opcode, type_name, mode) in [
        (19213, "HasBuffId", BuffConditionMode::Present),
        (57213, "NoBuffId", BuffConditionMode::Absent),
    ] {
        assert_eq!(
            parse(opcode, type_name, &["11790012".into()]),
            Some(ParsedConditionKind::BuffId {
                mode,
                buff_ids: vec![11790012],
            })
        );
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::HitPassives),
            })
        );
    }
}

#[test]
fn nested_no_action_teammate_death_keeps_its_exact_event_key() {
    assert_eq!(
        parse(17012, "TeammateDead", &[]),
        Some(ParsedConditionKind::TeammateDead)
    );
    assert_eq!(
        find_key(17012, "TeammateDead").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::EntityDied,
            phase: None,
        })
    );
    assert!(find_key(17013, "TeammateDead").is_none());
}

#[test]
fn hp_less_round_start_keys_keep_their_distinct_lanes() {
    assert_eq!(
        find_key(1103, "LifeLess").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
    );
    assert_eq!(
        find_key(1104, "LifeLess").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartLate,
            priority: 0,
        })
    );
}

#[test]
fn per_buff_id_skill_condition_keeps_exact_identity_and_stack_semantics() {
    assert_eq!(
        parse(
            59203,
            "PerBuffId",
            &["109320110".into(), "109320111".into()],
        ),
        Some(ParsedConditionKind::BuffIdCount {
            buff_ids: vec![109320110, 109320111],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert_eq!(
        find_key(59203, "PerBuffId").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert!(find_key(59203, "PerBuffIdCount").is_none());

    assert_eq!(
        find_key(59302, "PerBuffId").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEnd,
            phase: None,
        })
    );
    assert!(find_key(59302, "PerBuffIdCount").is_none());
}

#[test]
fn round_end_per_buff_id_count_keeps_one_repeat_per_layer() {
    assert_eq!(
        parse(61302, "PerBuffIdCount", &["116340011".into()]),
        Some(ParsedConditionKind::BuffIdCount {
            buff_ids: vec![116340011],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert_eq!(
        find_key(61302, "PerBuffIdCount").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEnd,
            phase: None,
        })
    );
    assert_eq!(
        parse(61302, "PerBuffIdCount", &["116340011".into(), "2".into()]),
        None
    );
}

#[test]
fn small_round_end_buff_and_status_gates_keep_separate_exact_keys() {
    assert_eq!(
        parse(19301, "HasBuffId", &["118353052".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![118353052],
        })
    );
    assert_eq!(
        parse(56301, "NoBuff", &["4".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![4],
            compare: ConditionCompare::Equal,
            threshold: 0,
        })
    );
    for (opcode, type_name) in [(19301, "HasBuffId"), (56301, "NoBuff")] {
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::SmallRoundEnd,
                phase: None,
            })
        );
    }
}

#[test]
fn after_settlement_buff_id_gate_keeps_its_exact_key() {
    assert_eq!(
        parse(19304, "HasBuffId", &["11410032".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![11410032],
        })
    );
    assert_eq!(
        find_key(19304, "HasBuffId").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEndAfterSettlement,
            phase: None,
        })
    );
}

#[test]
fn conduit_round_end_buff_id_gate_keeps_its_exact_key() {
    assert_eq!(
        parse(19307, "HasBuffId", &["8011".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![8011],
        })
    );
    assert_eq!(
        find_key(19307, "HasBuffId").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEndAfterSideFinalExpiry,
            phase: None,
        })
    );
}

#[test]
fn entity_settlement_buff_id_gate_keeps_owner_scope_and_exact_key() {
    assert_eq!(
        parse(19303, "HasBuffId", &["31460001".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![31460001],
        })
    );
    let definition = find_key(19303, "HasBuffId").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndEntitySettlement,
            phase: None,
        }
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    assert!(definition.filters_behavior_targets);
}

#[test]
fn accumulated_owner_buff_count_keeps_owner_scope_and_exact_key() {
    assert_eq!(
        parse(
            581,
            "AccAddBuffCountByBuffId",
            &["118353092".into(), "40".into()],
        ),
        Some(ParsedConditionKind::AccBuffAddedCount {
            buff_ids: vec![118353092],
            threshold: 40,
            scope: BuffAddedScope::Owner,
        })
    );
    let definition = find_key(581, "AccAddBuffCountByBuffId").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(
        definition.dependencies,
        &[EventKind::BuffAdded, EventKind::BuffChanged]
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
}

#[test]
fn final_settlement_buff_threshold_is_boolean_and_exact() {
    assert_eq!(
        parse(
            581307,
            "AccAddBuffCountByBuffId",
            &["118353082".into(), "5".into()],
        ),
        Some(ParsedConditionKind::BuffIdThreshold {
            buff_ids: vec![118353082],
            threshold: 5,
        })
    );
    assert_eq!(
        find_key(581307, "AccAddBuffCountByBuffId").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEndFinalSettlement,
            phase: None,
        })
    );
}

#[test]
fn power_ratio_keeps_resource_comparison_order_and_small_round_timing() {
    assert_eq!(
        parse(
            749301,
            "PowerRatio",
            &["9".into(), "1".into(), "1000".into()],
        ),
        Some(ParsedConditionKind::PowerRatio {
            power_id: 9,
            compare_code: 1,
            threshold_permille: 1000,
        })
    );
    assert_eq!(
        find_key(749301, "PowerRatio").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        })
    );
}

#[test]
fn entity_settlement_interval_keeps_period_then_start_order() {
    for (period, start) in [(2, 1), (2, 2)] {
        assert_eq!(
            parse(
                45303,
                "HeroRoundInterval",
                &[period.to_string(), start.to_string()],
            ),
            Some(ParsedConditionKind::RoundInterval {
                start_round: start,
                period,
            })
        );
    }
    let definition = find_key(45303, "HeroRoundInterval").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndEntitySettlement,
            phase: None,
        }
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
}

#[test]
fn round_end_interval_keeps_period_then_start_order() {
    for (period, start) in [(2, 1), (2, 2)] {
        assert_eq!(
            parse(
                45302,
                "HeroRoundInterval",
                &[period.to_string(), start.to_string()],
            ),
            Some(ParsedConditionKind::RoundInterval {
                start_round: start,
                period,
            })
        );
    }
    assert_eq!(
        find_key(45302, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEnd,
            phase: None,
        })
    );
}

#[test]
fn round_start_broken_checks_keep_their_exact_priorities() {
    for (opcode, priority) in [(783101, 101), (783102, 102)] {
        assert_eq!(
            parse(opcode, "IsBroken", &[]),
            Some(ParsedConditionKind::EntityBroken)
        );
        assert_eq!(
            find_key(opcode, "IsBroken").map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::RoundStartCondition,
                priority,
            })
        );
    }
}

#[test]
fn round_end_teammate_count_keeps_its_exact_scope_and_route() {
    assert_eq!(
        parse(73301, "TeammateAliveNum", &["0".into()]),
        Some(ParsedConditionKind::EntityCount {
            scope: super::super::super::parse::EntityCountScope::AliveOtherTeammates,
            compare: ConditionCompare::Equal,
            count: 0,
        })
    );
    let definition = find_key(73301, "TeammateAliveNum").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEnd,
            phase: None,
        }
    );
}

#[test]
fn active_skill_buff_count_keeps_its_exact_modifier_route() {
    assert_eq!(
        parse(61201, "PerBuffIdCount", &["109360002".into()]),
        Some(ParsedConditionKind::BuffIdCount {
            buff_ids: vec![109360002],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    let definition = find_key(61201, "PerBuffIdCount").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(
        definition.behavior_target_source,
        BehaviorTargetSource::ActiveSkillTargets
    );
}

#[test]
fn round_start_buff_count_keeps_its_exact_setup_route() {
    assert_eq!(
        parse(61102, "PerBuffIdCount", &["109360002".into()]),
        Some(ParsedConditionKind::BuffIdCount {
            buff_ids: vec![109360002],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert_eq!(
        find_key(61102, "PerBuffIdCount").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 102,
        })
    );
}

#[test]
fn player_buff_gate_keeps_side_presence_and_exact_buff_identity() {
    assert_eq!(
        parse(
            750101,
            "PlayerHasBuff",
            &["2".into(), "0".into(), "109320002".into()],
        ),
        Some(ParsedConditionKind::PlayerBuffPresence {
            present: false,
            buff_id: 109320002,
        })
    );
    assert_eq!(
        parse(
            750101,
            "PlayerHasBuff",
            &["1".into(), "0".into(), "109320002".into()],
        ),
        None
    );
    assert_eq!(
        find_key(750101, "PlayerHasBuff").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
}

#[test]
fn hopscotch_kill_count_uses_the_captured_after_hit_route() {
    assert_eq!(
        parse(992101, "PerKillNum", &["1".into()]),
        Some(ParsedConditionKind::PerKillCount { divisor: 1 })
    );
    assert_eq!(
        find_key(992101, "PerKillNum").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn hand_skill_presence_keeps_exact_card_identity_and_round_timing() {
    assert_eq!(
        parse(710301, "PerHandCardHasSkillId", &["118353040".into()],),
        Some(ParsedConditionKind::HandSkillPresence(vec![118353040]))
    );
    assert_eq!(
        find_key(710301, "PerHandCardHasSkillId").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        })
    );
}

#[test]
fn buff_type_totals_keep_their_exact_routes() {
    let expected = ParsedConditionKind::BuffTypeCount {
        type_ids: vec![31100201],
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: 4,
    };
    assert_eq!(
        parse(
            537201,
            "HasTypeIdBuffTotalCountMoreThan",
            &["31100201".into(), "4".into()],
        ),
        Some(expected)
    );
    assert_eq!(
        find_key(537201, "HasTypeIdBuffTotalCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
    assert_eq!(
        find_key(537203, "HasTypeIdBuffTotalCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        parse(
            537103,
            "HasTypeIdBuffTotalCountMoreThan",
            &["31210125".into(), "10".into()],
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![31210125],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 10,
        })
    );
    assert_eq!(
        find_key(537103, "HasTypeIdBuffTotalCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
    );
    assert!(
        parse(
            537201,
            "HasTypeIdBuffTotalCountMoreThan",
            &["0".into(), "4".into()]
        )
        .is_none()
    );
    assert!(
        parse(
            537203,
            "HasTypeIdBuffTotalCountMoreThan",
            &["31100201".into(), "0".into()]
        )
        .is_none()
    );
    assert!(
        parse(
            537103,
            "HasTypeIdBuffTotalCountMoreThan",
            &["31100201".into(), "8".into(), "1".into()]
        )
        .is_none()
    );
    assert!(find_key(537201, "TypeIdBuffCountMoreThan").is_none());
}

#[test]
fn ultimate_level_keeps_its_exact_round_start_route() {
    assert_eq!(
        parse(751104, "ExSkillLevel", &["0".into()]),
        Some(ParsedConditionKind::ExSkillLevel(0))
    );
    assert_eq!(
        find_key(751104, "ExSkillLevel").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartLate,
            priority: 0,
        })
    );
    assert!(parse(751104, "ExSkillLevel", &["-1".into()]).is_none());
    assert!(parse(751104, "ExSkillLevel", &["6".into()]).is_none());
    assert!(parse(751104, "ExSkillLevel", &["0".into(), "1".into()]).is_none());
    assert!(find_key(751104, "SkillLevel").is_none());
}

#[test]
fn after_hit_ultimate_level_condition_is_a_membership_predicate() {
    assert_eq!(
        parse(751210, "ExSkillLevel", &["0,1,2,3,4".into()]),
        Some(ParsedConditionKind::ExSkillLevels(vec![0, 1, 2, 3, 4]))
    );
    assert_eq!(
        find_key(751210, "ExSkillLevel").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    for raw in ["0,5", "4,3,2,1,0", "0,1,2,3,4,4", " 5 ", "0，1，2，3，4"] {
        assert!(parse(751210, "ExSkillLevel", &[raw.into()]).is_none());
    }
    for opcode in [751209, 751211, 751212] {
        assert!(find_key(opcode, "ExSkillLevel").is_none());
    }
}
