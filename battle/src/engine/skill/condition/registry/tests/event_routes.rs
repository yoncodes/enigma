use super::*;

#[test]
fn exact_life_less_uses_the_default_hp_lost_reaction_lane() {
    let definition = find_key(1, "LifeLess").unwrap();

    assert_eq!(definition.dependencies, &[EventKind::HpLost]);
    assert_eq!(
        definition.reaction_timing.resolve(Default::default()),
        ReactionTiming::Immediate
    );
}

#[test]
fn shared_damage_reacts_once_per_event_batch() {
    assert_eq!(
        find_key(695213, "ShareDamage").map(|definition| definition.reaction_multiplicity),
        Some(ReactionMultiplicity::OncePerEventBatch)
    );
}

#[test]
fn damage_hp_threshold_reactions_keep_their_owned_phase() {
    for opcode in [1209, 1304] {
        let definition = find_key(opcode, "LifeLess").unwrap();
        assert_eq!(
            definition.reaction_timing.resolve(Default::default()),
            ReactionTiming::HpThresholds
        );
        assert_eq!(
            definition.reaction_multiplicity,
            ReactionMultiplicity::OncePerAction
        );
    }
}

#[test]
fn generic_round_end_keys_keep_their_exact_timing() {
    assert_eq!(
        parse(301, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::SmallRoundEnd))
    );
    assert_eq!(
        parse(302, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::RoundEnd))
    );
    assert_eq!(
        parse(305, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::RoundEndAfterSettlement))
    );
}

#[test]
fn entity_entry_reactions_are_siblings_of_the_causing_event_skill() {
    assert_eq!(
        find_key(5, "EnterFight").map(|definition| definition.reaction_frame_scope),
        Some(ReactionFrameScope::EventSibling)
    );
}

#[test]
fn round_end_frames_target_the_passive_owner() {
    assert_eq!(
        find_key(302, "None").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
}

#[test]
fn replacement_round_end_reactions_own_an_independent_frame() {
    let definition = find_key(305, "None").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndAfterOwnerSettlement,
            phase: None,
        }
    );
    assert_eq!(
        definition.reaction_frame_scope,
        ReactionFrameScope::Independent
    );
    assert_eq!(
        find_key(306, "None").map(|definition| definition.reaction_frame_scope),
        Some(ReactionFrameScope::Subscriber)
    );
}

#[test]
fn entity_settlement_frames_target_the_settled_owner() {
    assert_eq!(
        find_key(303, "None").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
}

#[test]
fn shell_change_reactions_target_the_causing_frame() {
    for opcode in [214, 215] {
        assert_eq!(
            find_key(opcode, "None").map(|definition| definition.reaction_frame_target),
            Some(ReactionFrameTarget::CausingFrame)
        );
    }
}

#[test]
fn post_settlement_frames_target_the_passive_owner() {
    assert_eq!(
        find_key(304, "None").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
}

#[test]
fn accumulated_team_buff_frames_target_the_passive_owner() {
    assert_eq!(
        find_key(583004, "AccTeamAddBuffCountByBuffId")
            .map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
}

#[test]
fn cast_time_effect_tag_uses_the_pre_effect_event() {
    assert_eq!(
        find_key(34203, "UseSkillEffectTag").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillEffectStarted,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn source_opening_uses_its_own_pre_reaction_phase() {
    assert_eq!(
        find_key(201, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::SourceOpening),
        })
    );
    assert_eq!(
        find_key(203, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn post_damage_and_buff_id_conditions_keep_exact_route_roles() {
    assert_eq!(
        parse(208, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::SkillActionAfterDamage))
    );
    assert_eq!(
        find_key(208, "None").map(|definition| definition.behavior_target_source),
        Some(BehaviorTargetSource::HitTargets)
    );
    assert_eq!(
        find_key(210, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
    assert_eq!(
        parse(19004, "HasBuffId", &["30631".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![30631],
        })
    );
    assert_eq!(
        find_key(19004, "HasBuffId").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert_eq!(
        find_key(19004, "HasBuffId").map(|definition| definition.dependencies),
        Some(&[EventKind::BuffAdded, EventKind::BuffChanged][..])
    );
    assert!(parse(210, "HasBuffId", &[]).is_none());
}

#[test]
fn static_buff_id_conditions_keep_their_exact_dependencies() {
    for opcode in [19002, 19003] {
        assert_eq!(
            parse(opcode, "HasBuffId", &["1163852061".into()]),
            Some(ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![1163852061],
            })
        );
        assert_eq!(
            find_key(opcode, "HasBuffId").map(|definition| definition.role),
            Some(ConditionRole::Predicate)
        );
    }
    assert_eq!(
        find_key(19002, "HasBuffId").map(|definition| definition.dependencies),
        Some(&[][..])
    );
    assert_eq!(
        find_key(19003, "HasBuffId").map(|definition| definition.dependencies),
        Some(&[EventKind::BuffAdded, EventKind::BuffChanged][..])
    );
}

#[test]
fn regeneration_period_presence_gate_filters_the_source() {
    let definition = find_key(19012, "HasBuffId").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(19012, "HasBuffId", &["11410091".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![11410091],
        })
    );
}

#[test]
fn round_incantation_rank_count_keeps_its_exact_post_settlement_route() {
    assert_eq!(
        parse(622304, "RoundUseSkillLevel", &["2".into(), "2".into()],),
        Some(ParsedConditionKind::RoundUsedMinimumRank {
            minimum_rank: 2,
            threshold: 2,
        })
    );
    let definition = find_key(622304, "RoundUseSkillLevel").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndAfterSettlement,
            phase: None,
        }
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    assert!(find_key(622304, "ExPointMax").is_none());
}

#[test]
fn team_career_thresholds_keep_their_exact_post_settlement_routes() {
    for (opcode, type_name, compare, threshold) in [
        (
            562304,
            "CareerGroupHeroCountGE",
            ConditionCompare::GreaterThanOrEqual,
            2,
        ),
        (
            560304,
            "CareerGroupHeroCountLE",
            ConditionCompare::LessThanOrEqual,
            1,
        ),
    ] {
        assert_eq!(
            parse(opcode, type_name, &["4".into(), threshold.to_string()]),
            Some(ParsedConditionKind::TeamCareerCount {
                careers: vec![4],
                compare,
                threshold,
            })
        );
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(
            definition.role,
            ConditionRole::Trigger {
                event: EventKind::RoundEndAfterSettlement,
                phase: None,
            }
        );
        assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    }
}

#[test]
fn full_moxie_keeps_its_exact_post_settlement_route() {
    assert_eq!(
        parse(745304, "ExPointMax", &[]),
        Some(ParsedConditionKind::ExPointFull)
    );
    assert!(parse(745304, "ExPointMax", &["1".into()]).is_none());
    let definition = find_key(745304, "ExPointMax").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndAfterSettlement,
            phase: None,
        }
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    assert!(find_key(745304, "ExPointFull").is_none());
}

#[test]
fn follow_up_buff_gate_is_an_exact_inline_predicate() {
    let definition = find_key(19402, "HasBuffId").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(19402, "HasBuffId", &["30830111".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![30830111],
        })
    );
    assert!(find_key(19402, "NoBuffId").is_none());
}

#[test]
fn rejected_buff_gate_is_an_exact_transaction_trigger() {
    let definition = find_key(64208, "HasRejectBuffId").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::BuffRejected,
            phase: None,
        }
    );
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(64208, "HasRejectBuffId", &["4007".into()]),
        Some(ParsedConditionKind::RejectedBuffIdOrType(4007))
    );
    assert!(parse(64208, "HasRejectBuffId", &[]).is_none());
    assert!(parse(64208, "HasRejectBuffId", &["0".into()]).is_none());
    assert!(parse(64208, "HasRejectBuffId", &["4007".into(), "1".into()]).is_none());
    assert!(find_key(64208, "HasBuffId").is_none());
}

#[test]
fn regeneration_period_absence_gate_filters_the_source() {
    let definition = find_key(57012, "NoBuffId").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert!(definition.filters_behavior_targets);
    assert_eq!(
        parse(57012, "NoBuffId", &["11410091".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![11410091],
        })
    );
}

#[test]
fn riposte_buff_gate_is_an_exact_predicate() {
    let definition = find_key(19205, "HasBuffId").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert_eq!(
        definition.behavior_target_order,
        BehaviorTargetOrder::EntityIdentityMap
    );
    assert_eq!(
        parse(19205, "HasBuffId", &["5022".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::ExactPresent,
            buff_ids: vec![5022],
        })
    );
}

#[test]
fn missing_hp_multiplier_is_an_exact_predicate() {
    let definition = find_key(12203, "LostLifePer").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::HpLost]);
    assert_eq!(
        parse(12203, "LostLifePer", &["100".into()]),
        Some(ParsedConditionKind::PerLostHp {
            interval_permille: 100,
        })
    );
}

#[test]
fn hp_lost_ratio_keeps_its_exact_identity() {
    for opcode in [623203, 623204] {
        let definition = find_key(opcode, "HpLostRatio").unwrap();

        assert_eq!(definition.role, ConditionRole::Predicate);
        assert_eq!(definition.dependencies, &[EventKind::HpLost]);
        assert_eq!(
            parse(opcode, "HpLostRatio", &["100".into()]),
            Some(ParsedConditionKind::PerLostHp {
                interval_permille: 100,
            })
        );
        assert_eq!(parse(opcode, "LostLifePer", &["100".into()]), None);
        assert_eq!(parse(opcode, "HpLostRatio", &["0".into()]), None);
        assert_eq!(parse(opcode, "HpLostRatio", &["-100".into()]), None);
        assert_eq!(
            parse(opcode, "HpLostRatio", &["100".into(), "1".into()]),
            None
        );
    }
    assert_eq!(
        find_key(623203, "HpLostRatio").and_then(|definition| definition.attack_modifier_side),
        None
    );
    assert_eq!(
        find_key(623204, "HpLostRatio").and_then(|definition| definition.attack_modifier_side),
        Some(AttackModifierSide::IncomingTarget)
    );
}

#[test]
fn before_ap_resolution_keeps_its_exact_event_and_queue_preparation_roles() {
    let definition = find_key(107, "None").unwrap();

    assert_eq!(
        parse(107, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::BeforeApResolve))
    );
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::ActionQueueCommitted,
            phase: None,
        }
    );
    assert_eq!(
        definition.companion_setup,
        &[(SetupStage::GeneratedCard, 0)]
    );
    assert_eq!(
        parse(1061, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::ActionQueueCommitted))
    );
}

#[test]
fn conduit_group_selection_conditions_run_when_the_action_queue_is_committed() {
    for (opcode, type_name, args, expected) in [
        (
            19107,
            "HasBuffId",
            vec!["31450121".into()],
            ParsedConditionKind::BuffId {
                mode: crate::engine::skill::condition::buff::BuffConditionMode::Present,
                buff_ids: vec![31450121],
            },
        ),
        (
            794107,
            "DeviceSkillIndex",
            vec!["2".into()],
            ParsedConditionKind::ConduitSkillGroup { group: 2 },
        ),
    ] {
        let definition = find_key(opcode, type_name).unwrap();

        assert_eq!(parse(opcode, type_name, &args), Some(expected));
        assert_eq!(
            definition.role,
            ConditionRole::Trigger {
                event: EventKind::ActionQueueCommitted,
                phase: None,
            }
        );
    }
}

#[test]
fn lethal_injury_add_buff_condition_owns_the_immediate_skill_phase() {
    assert_eq!(
        parse(205, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::SkillActionStart))
    );
    assert_eq!(
        find_key(205, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
    assert!(find_key(205, "HasBuffId").is_none());
}

#[test]
fn after_attack_opcode_uses_the_skills_own_after_hit_phase() {
    assert_eq!(
        find_key(402, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
}

#[test]
fn burn_overflow_keys_keep_their_exact_skill_phases() {
    for (opcode, phase) in [
        (564203, SkillPhase::Immediate),
        (564210, SkillPhase::AfterHit),
    ] {
        assert_eq!(
            find_key(opcode, "BurnOverflow").map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(phase),
            })
        );
    }
}

#[test]
fn static_buff_gate_opcodes_have_distinct_setup_stages() {
    for (opcode, stage) in [(19103, SetupStage::BuffGate), (19104, SetupStage::BuffSync)] {
        let definition = find_key(opcode, "HasBuffId").unwrap();
        assert_eq!(definition.role, ConditionRole::Setup { stage, priority: 0 });
        assert!(definition.dependencies.is_empty());
    }
    assert_eq!(
        find_key(19104, "HasBuffId").map(|definition| definition.setup_frame_scope),
        Some(SetupFrameScope::Entity)
    );
}

#[test]
fn round_start_field_presence_keeps_its_exact_key_and_route() {
    let definition = find_key(542103, "InMagicCircleId").unwrap();

    assert_eq!(
        parse(542103, "InMagicCircleId", &["30003".into()]),
        Some(ParsedConditionKind::InMagicCircleId(vec![30003]))
    );
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        }
    );
    assert!(find_key(542103, "NotInMagicCircleId").is_none());
}

#[test]
fn post_ultimate_field_absence_uses_the_immediate_skill_action_route() {
    let definition = find_key(544203, "NotInMagicCircleId").unwrap();

    assert_eq!(
        parse(544203, "NotInMagicCircleId", &["42001".into()]),
        Some(ParsedConditionKind::NotInMagicCircleId(vec![42001]))
    );
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert!(find_key(544203, "InMagicCircleId").is_none());
}

#[test]
fn static_team_battle_tag_threshold_runs_at_battle_start() {
    for opcode in [762005, 762021] {
        assert_eq!(
            find_key(opcode, "BattleTagNum").map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::BattleStart,
                priority: 0,
            })
        );
    }
    assert_eq!(
        parse(
            762005,
            "BattleTagNum",
            &["113".into(), "3".into(), "1".into()]
        ),
        Some(ParsedConditionKind::BattleTagCount {
            tag_id: 113,
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert!(parse(762005, "Other", &["113".into(), "3".into(), "1".into()]).is_none());
    assert_eq!(
        find_key(762103, "BattleTagNum").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
    );
}

#[test]
fn received_hit_battle_tag_gate_checks_the_attacker_after_the_shared_hit() {
    let definition = find_key(1021209, "BattleTagCheck").unwrap();

    assert_eq!(
        parse(1021209, "BattleTagCheck", &["131".into()]),
        Some(ParsedConditionKind::TargetBattleTag(131))
    );
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::TargetAttacked]);
    assert_eq!(
        definition.reaction_timing.resolve(Default::default()),
        ReactionTiming::AfterSharedHit
    );
}

#[test]
fn next_round_buff_gate_runs_after_round_start() {
    let definition = find_key(19105, "HasBuffId").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::AfterRoundStart,
            priority: 0,
        }
    );
    assert_eq!(
        parse(19105, "HasBuffId", &["31490004".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![31490004],
        })
    );
}

#[test]
fn round_start_buff_type_threshold_runs_after_duration() {
    let definition = find_key(51104, "HasTypeIdBuffMoreThan").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 4,
        }
    );
    assert_eq!(definition.dependencies, &[EventKind::BuffChanged]);
}

#[test]
fn round_start_condition_buff_type_threshold_keeps_its_exact_lane() {
    assert_eq!(
        parse(
            51102,
            "HasTypeIdBuffMoreThan",
            &["30530102".into(), "5".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30530102],
            compare: super::super::super::ConditionCompare::GreaterThanOrEqual,
            threshold: 5,
        })
    );
    assert_eq!(
        find_key(51102, "HasTypeIdBuffMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 102,
        })
    );
    assert!(find_key(51102, "TypeIdBuffCountMoreThan").is_none());
}

#[test]
fn round_start_buff_type_threshold_keeps_its_entity_lane() {
    assert_eq!(
        parse(
            51103,
            "HasTypeIdBuffMoreThan",
            &["30530102".into(), "5".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30530102],
            compare: super::super::super::ConditionCompare::GreaterThanOrEqual,
            threshold: 5,
        })
    );
    let definition = find_key(51103, "HasTypeIdBuffMoreThan").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        }
    );
    assert_eq!(definition.setup_frame_scope, SetupFrameScope::Entity);
    assert!(find_key(51103, "TypeIdBuffCountMoreThan").is_none());
}

#[test]
fn round_start_unconditional_condition_keeps_its_entity_frame() {
    let definition = find_key(103, "None").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        }
    );
    assert_eq!(definition.setup_frame_scope, SetupFrameScope::Entity);
}

#[test]
fn small_round_end_buff_type_threshold_keeps_its_exact_route() {
    assert_eq!(
        parse(
            51301,
            "HasTypeIdBuffMoreThan",
            &["116330004".into(), "3".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![116330004],
            compare: super::super::super::ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    assert_eq!(
        find_key(51301, "HasTypeIdBuffMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        })
    );
}

#[test]
fn round_end_buff_type_threshold_keeps_its_exact_route() {
    let definition = find_key(51302, "HasTypeIdBuffMoreThan").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEnd,
            phase: None,
        }
    );
}

#[test]
fn dream_visit_threshold_runs_during_entity_settlement() {
    let definition = find_key(51303, "HasTypeIdBuffMoreThan").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndEntitySettlement,
            phase: None,
        }
    );
}

#[test]
fn dreamscape_active_skill_condition_runs_after_hit() {
    let definition = find_key(502210, "ActiveUseSkill").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        }
    );
}

#[test]
fn immediate_buff_type_threshold_observes_add_and_update() {
    assert_eq!(
        find_key(51213999, "HasTypeIdBuffMoreThan").map(|definition| definition.dependencies),
        Some(&[EventKind::BuffAdded, EventKind::BuffChanged][..])
    );
}

#[test]
fn buff_presence_action_opcodes_keep_their_exact_phases() {
    for (opcode, type_name, phase) in [
        (19203, "HasBuffId", SkillPhase::Immediate),
        (19208, "HasBuffId", SkillPhase::AfterDamage),
        (57208, "NoBuffId", SkillPhase::AfterDamage),
    ] {
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(
            definition.role,
            ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(phase),
            }
        );
        assert!(definition.dependencies.is_empty());
        assert_eq!(
            definition.behavior_target_order,
            BehaviorTargetOrder::EntityIdentityMap
        );
    }
}

#[test]
fn post_hit_buff_presence_conditions_are_filters_with_event_dependencies() {
    for (opcode, type_name) in [(19210, "HasBuffId"), (57210, "NoBuffId")] {
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(definition.role, ConditionRole::Predicate);
        assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
        assert!(definition.filters_behavior_targets);
    }
}

#[test]
fn genesis_critical_branch_uses_exact_static_buff_predicates() {
    for (opcode, type_name) in [(192081, "HasBuffId"), (572081, "NoBuffId")] {
        let definition = find_key(opcode, type_name).unwrap();
        assert_eq!(definition.role, ConditionRole::Predicate);
        assert!(definition.dependencies.is_empty());
        assert!(definition.filters_behavior_targets);
    }
}

#[test]
fn lethal_injury_skill_rate_gate_has_its_own_immediate_route() {
    let definition = find_key(51201, "HasTypeIdBuffMoreThan").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert_eq!(definition.read_timing, ConditionReadTiming::ActionStart);
    assert_eq!(
        find_key(51203, "HasTypeIdBuffMoreThan")
            .unwrap()
            .read_timing,
        ConditionReadTiming::Current
    );
    assert!(find_key(51201, "HasBuffId").is_none());
}

#[test]
fn conduit_attack_count_threshold_runs_after_hit() {
    let definition = find_key(51210, "HasTypeIdBuffMoreThan").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        }
    );
    assert_eq!(
        parse(
            51210,
            "HasTypeIdBuffMoreThan",
            &["31490008".into(), "3".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![31490008],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
}

#[test]
fn maximum_buff_layer_gate_is_an_exact_inline_predicate() {
    let definition = find_key(51212, "HasTypeIdBuffMoreThan").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert!(definition.dependencies.is_empty());
    assert_eq!(
        parse(
            51212,
            "HasTypeIdBuffMoreThan",
            &["30830161".into(), "12".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30830161],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 12,
        })
    );
    assert!(find_key(51212, "TypeIdBuffCountMoreThan").is_none());
}

#[test]
fn own_ultimate_use_fires_before_the_ultimate_damages() {
    assert_eq!(
        parse(25201, "UseExSkill", &[]),
        Some(ParsedConditionKind::UseExSkill)
    );
    assert_eq!(
        find_key(25201, "UseExSkill").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn paper_heron_ultimate_mode_uses_the_existing_immediate_route() {
    assert_eq!(
        parse(25203, "UseExSkill", &[]),
        Some(ParsedConditionKind::UseExSkill)
    );
    assert_eq!(
        parse(25203, "UseExSkill", &["3".into()]),
        Some(ParsedConditionKind::UseExSkill)
    );
    assert!(parse(25203, "UseExSkill", &["2".into()]).is_none());
    assert_eq!(
        find_key(25203, "UseExSkill").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
    assert!(parse(25201, "UseExSkill", &["3".into()]).is_none());
}

#[test]
fn trigger_families_reject_unconfigured_ids_and_wrong_types() {
    assert_eq!(
        parse(741402, "TriggerTypeBullet", &[]),
        Some(ParsedConditionKind::BuffFeatureTriggered { act_id: 827 })
    );
    assert!(parse(741999, "TriggerTypeBullet", &[]).is_none());
    assert!(parse(25210, "BeAttacked", &[]).is_none());
    assert_eq!(
        find_key(25210, "UseExSkill").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        })
    );
    assert_eq!(
        find_key(22213, "BeAttacked").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::HitPassives),
        })
    );
    assert_eq!(
        find_key(22213, "BeAttacked").map(|definition| definition.skill_action_observer),
        Some(SkillActionObserver::AllyOfAttackedTarget)
    );
    assert_eq!(
        find_key(22213, "BeAttacked").map(|definition| definition.publication),
        Some(PublicationPhase::BeforePublish)
    );
    assert_eq!(
        find_key(1001212, "Assassinate").map(|definition| {
            (
                definition.role,
                definition.skill_action_observer,
                definition.reaction_frame_target,
            )
        }),
        Some((
            ConditionRole::Trigger {
                event: EventKind::AllyAction,
                phase: None,
            },
            SkillActionObserver::Team,
            ReactionFrameTarget::CausingFrame,
        ))
    );
    assert_eq!(
        find_key(1000212, "TeamContainHero").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
}

#[test]
fn unregistered_family_parsers_do_not_bypass_exact_lookup() {
    for (opcode, type_name, args) in [
        (45, "HeroRoundInterval", vec!["1".into()]),
        (19, "HasBuffId", vec!["101".into()]),
        (51, "HasTypeIdBuffMoreThan", vec!["101".into(), "1".into()]),
        (6208, "UseSkillStar", vec!["2".into()]),
        (616208, "TeammateAliveNumNoSp", vec!["2".into()]),
    ] {
        assert!(parse(opcode, type_name, &args).is_none());
    }
    assert_eq!(
        parse(45102, "HeroRoundInterval", &["1".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 1,
            period: 0,
        })
    );
}
