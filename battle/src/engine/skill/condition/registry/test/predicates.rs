use super::*;

#[test]
fn attacked_afflatus_conditions_keep_their_exact_event_route() {
    for (opcode, type_name) in [(33209, "HurtRestraint"), (47209, "HurtNotRestraint")] {
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(definition.role, ConditionRole::Predicate);
        assert_eq!(definition.dependencies, &[EventKind::TargetAttacked]);
        assert_eq!(definition.attack_modifier_side, None);
    }

    assert!(find_key(33209, "HurtNotRestraint").is_none());
    assert!(find_key(47209, "HurtRestraint").is_none());
}

#[test]
fn restrained_ultimate_conditions_keep_their_exact_action_roles() {
    let ultimate = find_key(25204, "UseExSkill").unwrap();
    assert_eq!(
        ultimate.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert_eq!(
        ultimate.skill_action_observer,
        SkillActionObserver::AttackTarget
    );

    let restrained = find_key(33204, "HurtRestraint").unwrap();
    assert_eq!(
        restrained.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert_eq!(
        restrained.skill_action_observer,
        SkillActionObserver::AttackTarget
    );
}

#[test]
fn received_hit_afflatus_conditions_keep_their_exact_event_lane() {
    for (opcode, type_name) in [(33209, "HurtRestraint"), (47209, "HurtNotRestraint")] {
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(definition.role, ConditionRole::Predicate);
        assert_eq!(definition.dependencies, &[EventKind::TargetAttacked]);
        assert_eq!(definition.attack_modifier_side, None);
    }
}

#[test]
fn bound_pair_threshold_keeps_its_per_entity_predicate() {
    assert_eq!(
        parse(
            535212,
            "TypeIdBuffCountMoreThan",
            &["31000303".into(), "8".into()],
        ),
        Some(ParsedConditionKind::AnyTargetBuffTypeCount {
            type_ids: vec![31000303],
            threshold: 8,
        })
    );
    assert_eq!(
        find_key(535212, "TypeIdBuffCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert!(find_key(535212, "HasTypeIdBuffMoreThan").is_none());
}

#[test]
fn distinct_status_type_count_keeps_its_exact_predicate_route() {
    let definition = find_key(85203, "PerBuffTypeCountGroupByTypeId").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(
        (definition.parse)(
            85203,
            "PerBuffTypeCountGroupByTypeId",
            &["1,3,5".into(), "14".into()],
        ),
        Some(ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids: vec![1, 3, 5, 14],
            divisor: 1,
            max_count: i32::MAX,
        })
    );
    assert!(find_key(85203, "PerBuffTypeCountGroupByTypeIdLimit").is_none());
}

#[test]
fn static_status_predicate_keeps_its_exact_source_side_route() {
    let definition = find_key(18201, "HasBuff").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(definition.attack_modifier_side, None);
    assert_eq!(
        parse(18201, "HasBuff", &["1".into(), "5".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![1, 5],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    assert!(find_key(18201, "HasBuffId").is_none());
}

#[test]
fn after_damage_status_condition_filters_behavior_targets() {
    let definition = find_key(18208, "HasBuff").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterDamage),
        }
    );
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(18208, "HasBuff", &["2".into(), "4".into(), "6".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![2, 4, 6],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
}

#[test]
fn control_status_round_start_gate_keeps_its_setup_lane() {
    assert_eq!(
        parse(18301, "HasBuff", &["4".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![4],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        })
    );
    let definition = find_key(18301, "HasBuff").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        }
    );
    assert!(definition.filters_behavior_targets);
}

#[test]
fn missing_buff_incoming_modifier_keeps_exact_identity() {
    assert_eq!(
        parse(57204, "NoBuffId", &["22300341".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![22300341],
        })
    );
    let definition = find_key(57204, "NoBuffId").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        definition.attack_modifier_side,
        Some(AttackModifierSide::IncomingTarget)
    );
}

#[test]
fn active_skill_buff_threshold_is_inclusive() {
    assert_eq!(
        parse(
            535201,
            "TypeIdBuffCountMoreThan",
            &["30830111".into(), "6".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30830111],
            compare: super::super::super::parse::ConditionCompare::GreaterThanOrEqual,
            threshold: 6,
        })
    );
    assert_eq!(
        find_key(535201, "TypeIdBuffCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn child_skill_buff_threshold_stays_a_predicate() {
    assert_eq!(
        parse(
            535304,
            "TypeIdBuffCountMoreThan",
            &["116385672".into(), "5".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![116385672],
            compare: super::super::super::parse::ConditionCompare::GreaterThanOrEqual,
            threshold: 5,
        })
    );

    let definition = find_key(535304, "TypeIdBuffCountMoreThan").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(find_key(535304, "TypeIdBuffCountLessThan").is_none());
    assert_eq!(
        parse(
            535304,
            "TypeIdBuffCountMoreThan",
            &["116385672".into(), "5".into(), "999".into()]
        ),
        None
    );
}

#[test]
fn active_skill_buff_ceiling_is_inclusive() {
    assert_eq!(
        parse(
            536201,
            "TypeIdBuffCountLessThan",
            &["30830111".into(), "5".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30830111],
            compare: super::super::super::parse::ConditionCompare::LessThanOrEqual,
            threshold: 5,
        })
    );
    assert_eq!(
        find_key(536201, "TypeIdBuffCountLessThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn active_skill_enemy_count_includes_special_entities() {
    assert_eq!(
        parse(548201, "EnemyNumIncludeSpEqual", &["2".into()]),
        Some(ParsedConditionKind::EntityCount {
            scope: super::super::super::parse::EntityCountScope::AliveEnemiesIncludeSp,
            compare: super::super::super::parse::ConditionCompare::Equal,
            count: 2,
        })
    );
    assert_eq!(
        find_key(548201, "EnemyNumIncludeSpEqual").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn conduit_switch_enemy_counts_run_after_the_skill_hits() {
    for (opcode, type_name, compare) in [
        (
            546210,
            "EnemyNumIncludeSpMoreThan",
            ConditionCompare::GreaterThanOrEqual,
        ),
        (548210, "EnemyNumIncludeSpEqual", ConditionCompare::Equal),
    ] {
        assert_eq!(
            parse(opcode, type_name, &["1".into()]),
            Some(ParsedConditionKind::EntityCount {
                scope: super::super::super::parse::EntityCountScope::AliveEnemiesIncludeSp,
                compare,
                count: 1,
            })
        );
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            })
        );
    }
}

#[test]
fn after_damage_status_threshold_checks_the_actor() {
    let definition = find_key(42208, "HasTypeBuffMoreThan").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterDamage),
        }
    );
    assert!(!definition.filters_behavior_targets);
    assert_eq!(
        parse(42208, "HasTypeBuffMoreThan", &["2".into(), "1,5".into()]),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![1, 5],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 2,
        })
    );
}

#[test]
fn active_skill_status_thresholds_preserve_strictness() {
    assert_eq!(
        parse(42203, "HasTypeBuffMoreThan", &["6".into(), "1,5".into()]),
        Some(ParsedConditionKind::VisibleStackedStatusTypeCount {
            status_ids: vec![1, 5],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 6,
        })
    );
    assert_eq!(
        parse(43203, "HasTypeBuffLessThan", &["6".into(), "1,5".into()]),
        Some(ParsedConditionKind::VisibleBuffStatusTypeCount {
            status_ids: vec![1, 5],
            compare: crate::engine::skill::condition::ConditionCompare::LessThan,
            threshold: 6,
        })
    );
    for (opcode, type_name) in [
        (42203, "HasTypeBuffMoreThan"),
        (43203, "HasTypeBuffLessThan"),
    ] {
        assert_eq!(
            find_key(opcode, type_name).unwrap().behavior_target_source,
            BehaviorTargetSource::Resolved
        );
    }

    assert_eq!(
        find_key(42203, "HasTypeBuffMoreThan").unwrap().role,
        ConditionRole::Trigger {
            event: EventKind::SkillEffectStarted,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert!(
        find_key(42203, "HasTypeBuffMoreThan")
            .unwrap()
            .skill_action_trigger
            .requires_attack()
    );
    assert_eq!(
        find_key(43203, "HasTypeBuffLessThan").unwrap().role,
        ConditionRole::Predicate
    );
}

#[test]
fn post_hit_status_ceiling_is_inclusive() {
    assert_eq!(
        parse(
            512210,
            "HasTypeBuffIdsLessThan",
            &["2".into(), "2,4,6".into()]
        ),
        Some(ParsedConditionKind::BuffStatusCount {
            status_ids: vec![2, 4, 6],
            compare: crate::engine::skill::condition::ConditionCompare::LessThanOrEqual,
            threshold: 2,
        })
    );
    assert_eq!(
        find_key(512210, "HasTypeBuffIdsLessThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn team_status_type_groups_keep_divisor_cap_and_categories() {
    assert_eq!(
        parse(
            539301,
            "PerSelfTeamTypeType2BuffTypeIdNum",
            &["3".into(), "5".into(), "1,3,5,7,14".into()]
        ),
        Some(ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids: vec![1, 3, 5, 7, 14],
            divisor: 3,
            max_count: 5,
        })
    );
    assert_eq!(
        find_key(539301, "PerSelfTeamTypeType2BuffTypeIdNum").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        })
    );
}

#[test]
fn defender_action_phase_fight_progress_gate_keeps_its_exact_route_and_payload_order() {
    assert_eq!(
        parse(
            1010301,
            "ProgressValue",
            &["8".into(), "3".into(), "100".into()]
        ),
        Some(ParsedConditionKind::FightProgressValue {
            progress_id: 8,
            compare_code: 3,
            threshold: 100,
        })
    );
    assert_eq!(
        find_key(1010301, "ProgressValue").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::DefenderActionPhaseStart,
            phase: None,
        })
    );
    assert_eq!(
        find_key(1010301, "ProgressValue").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
    assert!(find_key(1010101, "ProgressValue").is_none());
    assert_eq!(
        find_key(1010012, "ProgressValue").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::EntityDied,
            phase: None,
        })
    );
    assert_eq!(
        parse(
            1010301,
            "ProgressValue",
            &["8".into(), "4".into(), "100".into()]
        ),
        None
    );
}

#[test]
fn synchronization_reaction_owns_its_independent_self_targeted_frame() {
    let definition = find_key(1008101, "Synchronization").unwrap();

    assert_eq!(definition.dependencies, &[EventKind::ExPointChanged]);
    assert_eq!(
        definition.reaction_frame_scope,
        ReactionFrameScope::IndependentOutsideAction
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
}

#[test]
fn active_team_status_type_count_keeps_its_exact_predicate_route() {
    let definition = find_key(539203, "PerSelfTeamTypeType2BuffTypeIdNum").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(
        parse(
            539203,
            "PerSelfTeamTypeType2BuffTypeIdNum",
            &["1".into(), "99".into(), "6".into()]
        ),
        Some(ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids: vec![6],
            divisor: 1,
            max_count: 99,
        })
    );
    for arguments in [
        vec!["0".into(), "99".into(), "6".into()],
        vec!["1".into(), "0".into(), "6".into()],
        vec!["1".into(), "99".into(), "0".into()],
        vec!["1".into(), "99".into(), "6".into(), "7".into()],
    ] {
        assert_eq!(
            parse(539203, "PerSelfTeamTypeType2BuffTypeIdNum", &arguments),
            None
        );
    }
    assert!(find_key(539203, "PerBuffTypeCountGroupByTypeId").is_none());
}

#[test]
fn active_buff_group_count_keeps_its_exact_predicate_route() {
    let definition = find_key(669203, "PerBuffGroupCount").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(
        definition.behavior_target_source,
        BehaviorTargetSource::ActiveSkillTargetGroup
    );
    assert_eq!(
        parse(669203, "PerBuffGroupCount", &["7".into()]),
        Some(ParsedConditionKind::PerBuffGroupCount { group_id: 7 })
    );
    for arguments in [
        vec![],
        vec!["0".into()],
        vec!["-7".into()],
        vec!["7".into(), "5".into()],
    ] {
        assert_eq!(parse(669203, "PerBuffGroupCount", &arguments), None);
    }
    assert!(find_key(669203, "HasBuffGroup").is_none());
}

#[test]
fn bullet_type_count_keeps_its_exact_active_modifier_route() {
    let definition = find_key(651203, "PerBullet").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
    assert_eq!(
        definition.behavior_target_source,
        BehaviorTargetSource::ActiveSkillTargets
    );
    assert_eq!(
        parse(651203, "PerBullet", &["1".into(), "8".into()]),
        Some(ParsedConditionKind::PerBullet {
            divisor: 1,
            max_count: 8,
        })
    );
    for arguments in [
        vec![],
        vec!["1".into()],
        vec!["0".into(), "8".into()],
        vec!["1".into(), "0".into()],
        vec!["1".into(), "8".into(), "99".into()],
    ] {
        assert_eq!(parse(651203, "PerBullet", &arguments), None);
    }
    assert!(find_key(651203, "PerBuffGroupCount").is_none());
}

#[test]
fn bullet_type_count_keeps_its_exact_after_hit_route() {
    let definition = find_key(651210, "PerBullet").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        }
    );
    assert_eq!(
        definition.behavior_target_source,
        BehaviorTargetSource::ActiveSkillTargets
    );
    assert_eq!(
        parse(651210, "PerBullet", &["1".into(), "99".into()]),
        Some(ParsedConditionKind::PerBullet {
            divisor: 1,
            max_count: 99,
        })
    );
    assert!(find_key(651210, "PerBuffIdCount").is_none());
}

#[test]
fn firebud_rank_gate_uses_the_exact_after_damage_lane() {
    for (opcode, type_name) in [(66208, "UseSpecificSkill"), (501208, "UseHurtSkill")] {
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterDamage),
            })
        );
    }
    assert_eq!(
        parse(66208, "UseSpecificSkill", &["5".to_owned(), "3".to_owned()]),
        Some(ParsedConditionKind::SpecificSkill { group: 5, rank: 3 })
    );
    assert_eq!(
        parse(501208, "UseHurtSkill", &[]),
        Some(ParsedConditionKind::UseHurtSkill)
    );
}

#[test]
fn damaging_skill_post_hit_gate_keeps_its_exact_lane() {
    assert_eq!(
        parse(501210, "UseHurtSkill", &[]),
        Some(ParsedConditionKind::UseHurtSkill)
    );
    assert_eq!(
        find_key(501210, "UseHurtSkill").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn ally_incantation_rank_uses_the_exact_ally_action_lane() {
    assert_eq!(
        find_key(620212, "CurrSkillLevel").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        })
    );
    assert_eq!(
        parse(620212, "CurrSkillLevel", &["1".to_owned()]),
        Some(ParsedConditionKind::ActiveSkillRank {
            compare: crate::engine::skill::condition::ConditionCompare::Equal,
            ranks: vec![1],
        })
    );
}

#[test]
fn ultimate_blood_pool_threshold_is_an_exact_static_predicate() {
    let definition = find_key(726210, "BloodPoolValue").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(find_key(726211, "BloodPoolValue").is_none());
}

#[test]
fn attack_crit_after_damage_route_is_exact() {
    assert_eq!(
        find_key(30208, "AttackCrit").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterDamage),
        })
    );
    assert_eq!(
        find_key(30402, "AttackCrit").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterDamage),
        })
    );
    assert_eq!(
        find_key(30210, "AttackCrit").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        find_key(30208, "AttackCrit").map(|definition| definition.skill_action_trigger),
        Some(SkillActionTriggerPolicy::Default)
    );
    assert_eq!(
        find_key(30402, "AttackCrit").map(|definition| definition.skill_action_trigger),
        Some(SkillActionTriggerPolicy::InitialCriticalFollowUp)
    );
    assert_eq!(
        find_key(30402, "AttackCrit")
            .and_then(|definition| definition.skill_action_trigger.child_skill_kind()),
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp)
    );
    assert!(find_key(30402, "AttackCrit").is_some_and(|definition| {
        definition.skill_action_trigger.rejects_extra_skill(
            crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp.id(),
        )
    }));
    assert!(find_key(30208, "AttackCrit").is_some_and(|definition| {
        !definition.skill_action_trigger.rejects_extra_skill(
            crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp.id(),
        )
    }));
    assert_eq!(
        find_key(30210, "AttackCrit").map(|definition| definition.skill_action_trigger),
        Some(SkillActionTriggerPolicy::Default)
    );
    assert!(find_key(30209, "AttackCrit").is_none());
}

#[test]
fn before_crit_has_its_own_pre_damage_route() {
    assert_eq!(
        parse(7203, "BeforeCrit", &[]),
        Some(ParsedConditionKind::BeforeCrit)
    );
    assert_eq!(
        find_key(7203, "BeforeCrit").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Damage),
        })
    );
    assert!(find_key(7204, "BeforeCrit").is_none());
}

#[test]
fn active_life_more_gate_keeps_its_exact_static_route() {
    assert_eq!(
        find_key(2203, "LifeMore").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        parse(2203, "LifeMore", &["500".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::super::parse::ConditionCompare::GreaterThan,
            threshold: 500,
        })
    );
    assert!(find_key(2204, "LifeMore").is_none());
}

#[test]
fn hp_loss_life_less_gate_keeps_its_exact_identity() {
    assert_eq!(
        find_key(1, "LifeLess").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        find_key(1, "LifeLess").map(|definition| definition.dependencies),
        Some([EventKind::HpLost].as_slice())
    );
    assert_eq!(
        find_key(1, "LifeLess").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
    assert_eq!(
        parse(1, "LifeLess", &["11".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::super::parse::ConditionCompare::LessThan,
            threshold: 11,
        })
    );
}

#[test]
fn round_end_life_more_gate_uses_a_strict_threshold() {
    assert_eq!(
        parse(2301, "LifeMore", &["900".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::super::parse::ConditionCompare::GreaterThan,
            threshold: 900,
        })
    );
    assert_eq!(
        find_key(2301, "LifeMore").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        })
    );
}

#[test]
fn round_start_hp_gates_keep_their_exact_setup_stages() {
    for (opcode, type_name, stage) in [
        (1104, "LifeLess", SetupStage::RoundStartLate),
        (1105, "LifeLess", SetupStage::AfterRoundStart),
        (2104, "LifeMore", SetupStage::RoundStartLate),
    ] {
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Setup { stage, priority: 0 })
        );
    }
    assert_eq!(
        parse(2104, "LifeMore", &["500".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::super::parse::ConditionCompare::GreaterThan,
            threshold: 500,
        })
    );
}

#[test]
fn incoming_life_less_gate_keeps_its_exact_modifier_route() {
    assert_eq!(
        parse(1204, "LifeLess", &["500".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::super::parse::ConditionCompare::LessThan,
            threshold: 500,
        })
    );
    assert!(find_key(2204, "LifeLess").is_none());
}

#[test]
fn per_buff_type_layer_is_an_exact_static_multiplier() {
    assert_eq!(
        parse(
            518203,
            "PerHasBuffTypeLayer",
            &["1".into(), "100".into(), "31280114".into()]
        ),
        Some(ParsedConditionKind::PerBuffTypeLayer {
            type_ids: vec![31280114],
            min: 1,
            max: 100,
        })
    );
    assert_eq!(
        find_key(518203, "PerHasBuffTypeLayer").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert!(find_key(518204, "PerHasBuffTypeLayer").is_none());
}

#[test]
fn post_hit_buff_layer_conversion_keeps_its_repeat_cap() {
    assert_eq!(
        parse(
            518210,
            "PerHasBuffTypeLayer",
            &["1".into(), "3".into(), "31100147".into()]
        ),
        Some(ParsedConditionKind::PerBuffTypeLayer {
            type_ids: vec![31100147],
            min: 1,
            max: 3,
        })
    );
    assert_eq!(
        find_key(518210, "PerHasBuffTypeLayer").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn round_start_team_buff_type_gate_keeps_its_exact_payload_order() {
    assert_eq!(
        parse(
            514100,
            "SelfTeamHasBuffTypeLayerLessThan",
            &["0".into(), "30650104".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30650104],
            compare: super::super::super::parse::ConditionCompare::LessThanOrEqual,
            threshold: 0,
        })
    );
    assert_eq!(
        find_key(514100, "SelfTeamHasBuffTypeLayerLessThan").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 100,
        })
    );
    assert!(find_key(514101, "SelfTeamHasBuffTypeLayerLessThan").is_none());
}

#[test]
fn round_start_power_gate_keeps_its_exact_phase_and_payload_order() {
    assert_eq!(
        parse(
            180100,
            "PowerCompare",
            &["1".into(), "11".into(), "5".into()]
        ),
        Some(ParsedConditionKind::PowerCompare {
            compare_code: 1,
            power_id: 11,
            threshold: 5,
        })
    );
    assert_eq!(
        find_key(180100, "PowerCompare").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 100,
        })
    );
    assert_eq!(
        parse(
            180102,
            "PowerCompare",
            &["1".into(), "11".into(), "2".into()]
        ),
        Some(ParsedConditionKind::PowerCompare {
            compare_code: 1,
            power_id: 11,
            threshold: 2,
        })
    );
    assert_eq!(
        find_key(180102, "PowerCompare").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 102,
        })
    );
    assert!(find_key(180101, "PowerCompare").is_none());
}

#[test]
fn active_device_group_gate_keeps_its_exact_after_hit_route() {
    assert_eq!(
        parse(794210, "DeviceSkillIndex", &["1".into()]),
        Some(ParsedConditionKind::ConduitSkillGroup { group: 1 })
    );
    assert_eq!(
        find_key(794210, "DeviceSkillIndex").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
    assert!(find_key(794209, "DeviceSkillIndex").is_none());
}
