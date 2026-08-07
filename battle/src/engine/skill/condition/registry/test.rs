use super::*;
use crate::engine::skill::condition::{
    ConditionCompare, buff::BuffConditionMode, none::NoneMode, parse::BuffAddedScope,
};

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
        Some(ParsedConditionKind::None(NoneMode::RoundEnd))
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
fn static_team_battle_tag_threshold_runs_at_battle_start() {
    assert_eq!(
        find_key(762021, "BattleTagNum").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::BattleStart,
            priority: 0,
        })
    );
    assert_eq!(
        find_key(762103, "BattleTagNum").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
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
            compare: super::super::ConditionCompare::GreaterThanOrEqual,
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
            compare: super::super::ConditionCompare::GreaterThanOrEqual,
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
fn round_start_none_103_keeps_its_entity_frame() {
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
        find_key(1001212, "Assassinate")
            .map(|definition| { (definition.role, definition.skill_action_observer) }),
        Some((
            ConditionRole::Trigger {
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            },
            SkillActionObserver::Team,
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

#[test]
fn hero_round_interval_keeps_its_exact_round_transition_lane() {
    assert_eq!(
        parse(45102, "HeroRoundInterval", &["4".into(), "1".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 4,
            period: 1,
        })
    );
    assert_eq!(
        parse(45104, "HeroRoundInterval", &["2".into(), "1".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 1,
            period: 2,
        })
    );
    assert_eq!(
        parse(45104, "HeroRoundInterval", &["2".into(), "2".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 2,
            period: 2,
        })
    );
    assert_eq!(parse(45104, "HeroRoundInterval", &["2".into()]), None);
    assert_eq!(
        find_key(45102, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundTransitionStart,
            priority: 0,
        })
    );
    assert_eq!(
        find_key(45104, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundTransitionStart,
            priority: 1,
        })
    );
}

#[test]
fn card_setup_interval_keeps_its_exact_lane() {
    assert_eq!(
        parse(45106, "HeroRoundInterval", &["4".into(), "2".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 2,
            period: 4,
        })
    );
    assert_eq!(
        find_key(45106, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::CardSetup,
            priority: 0,
        })
    );
}

#[test]
fn round_start_interval_keeps_its_exact_lane() {
    assert_eq!(
        parse(45100, "HeroRoundInterval", &["1".into(), "5".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 5,
            period: 1,
        })
    );
    assert_eq!(
        find_key(45100, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: -1,
        })
    );
    assert!(parse(45100, "Other", &["1".into(), "5".into()]).is_none());
}

#[test]
fn none_round_start_opcodes_keep_their_exact_lanes() {
    assert_eq!(
        find_key(100, "None").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: -1,
        })
    );
    for opcode in [101, 102] {
        assert_eq!(
            find_key(opcode, "None").map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::RoundStartCondition,
                priority: opcode,
            })
        );
    }
    assert_eq!(
        find_key(103, "None").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
    );
    assert_eq!(
        find_key(104, "None").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartLate,
            priority: 0,
        })
    );
}

#[test]
fn missing_buff_round_end_gate_keeps_the_actor_target() {
    assert_eq!(
        parse(57301, "NoBuffId", &["90171".into()]),
        Some(ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![90171],
        })
    );
    let definition = find_key(57301, "NoBuffId").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SmallRoundEnd,
            phase: None,
        }
    );
    assert!(definition.filters_behavior_targets);
}

#[test]
fn conditional_round_start_interval_preserves_period_then_start_order() {
    assert_eq!(
        parse(45101, "HeroRoundInterval", &["99".into(), "15".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 15,
            period: 99,
        })
    );
    assert_eq!(
        find_key(45101, "HeroRoundInterval").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
}

#[test]
fn round_after_uses_an_inclusive_round_start_threshold() {
    assert_eq!(
        parse(727100, "RoundAfter", &["4".into()]),
        Some(ParsedConditionKind::RoundInterval {
            start_round: 4,
            period: 1,
        })
    );
    assert_eq!(parse(727100, "RoundAfter", &[]), None);
    assert_eq!(
        find_key(727100, "RoundAfter").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 100,
        })
    );
}

#[test]
fn magic_circle_round_start_key_keeps_its_setup_lane() {
    assert_eq!(
        parse(542103, "InMagicCircleId", &["30003".into()]),
        Some(ParsedConditionKind::InMagicCircleId(vec![30003]))
    );
    assert_eq!(
        find_key(542103, "InMagicCircleId").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStart,
            priority: 1,
        })
    );
    assert!(parse(542103, "Other", &["30003".into()]).is_none());
}

#[test]
fn exact_dead_alias_subscribes_to_entity_death() {
    assert_eq!(
        parse(812, "Dead", &[]),
        Some(ParsedConditionKind::EntityDead)
    );
    assert_eq!(
        find_key(812, "Dead").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::EntityDied,
            phase: None,
        })
    );
    assert_eq!(
        find_key(812, "Dead").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
}

#[test]
fn exact_alive_team_count_rechecks_on_entity_death() {
    assert!(matches!(
        parse(616012, "TeammateAliveNumNoSp", &["0".into()]),
        Some(ParsedConditionKind::EntityCount {
            scope: super::super::parse::EntityCountScope::AliveTeammatesNoSp,
            compare: super::super::parse::ConditionCompare::Equal,
            count: 0,
        })
    ));
    assert_eq!(
        find_key(616012, "TeammateAliveNumNoSp").map(|definition| definition.dependencies),
        Some(&[EventKind::EntityDied][..])
    );
    assert!(find_key(616208, "TeammateAliveNumNoSp").is_none());
}

#[test]
fn exact_enter_battle_none_alias_waits_for_an_entity_entry() {
    assert_eq!(
        parse(55, "None", &[]),
        Some(ParsedConditionKind::None(NoneMode::EnterBattle))
    );
    assert_eq!(
        find_key(55, "None").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::EntityEntered,
            phase: None,
        })
    );
}

#[test]
fn multi_hp_segment_is_a_predicate_not_a_second_trigger() {
    assert_eq!(
        parse(510, "MultiHpXIn", &["2".into()]),
        Some(ParsedConditionKind::MultiHpSegment(2))
    );
    assert_eq!(
        find_key(510, "MultiHpXIn").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
}

#[test]
fn resource_change_reactions_use_their_exact_publication_phase() {
    assert_eq!(
        parse(40, "LostExPoint", &["0".into()]),
        Some(ParsedConditionKind::ExPointLost)
    );
    assert_eq!(
        find_key(40, "LostExPoint").map(|definition| definition.publication),
        Some(PublicationPhase::AfterPublish)
    );
    assert_eq!(
        parse(566, "PowerUseAddBuff", &["2".into()]),
        Some(ParsedConditionKind::PowerUseAddBuff { threshold: 2 })
    );
    assert_eq!(
        find_key(566, "PowerUseAddBuff").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::EurekaChanged,
            phase: None,
        })
    );
    assert_eq!(
        find_key(566, "PowerUseAddBuff").map(|definition| definition.publication),
        Some(PublicationPhase::AfterPublish)
    );
    assert_eq!(
        find_key(566, "PowerUseAddBuff").map(|definition| definition.reaction_timing),
        Some(ReactionTiming::AfterSkill)
    );
    assert_eq!(
        find_key(566, "PowerUseAddBuff").map(|definition| definition.reaction_frame_target),
        Some(ReactionFrameTarget::Owner)
    );
    assert_eq!(
        find_key(660008, "PerDecrExPoint").map(|definition| definition.publication),
        Some(PublicationPhase::BeforePublish)
    );
    assert_eq!(
        find_key(579018, "ExPointIncrChange").map(|definition| definition.publication),
        Some(PublicationPhase::BeforePublish)
    );
    assert_eq!(
        find_key(579023, "ExPointIncrChange").map(|definition| definition.publication),
        Some(PublicationPhase::BeforePublish)
    );
    for opcode in [579018, 579023] {
        assert_eq!(
            find_key(opcode, "ExPointIncrChange")
                .map(|definition| definition.reaction_frame_target),
            Some(ReactionFrameTarget::Owner)
        );
    }
    assert_eq!(
        parse(579018, "ExPointIncrChange", &["1".into(), "0".into()]),
        Some(ParsedConditionKind::ExPointIncrChange {
            threshold: 1,
            kind: 0,
            scope: crate::engine::skill::condition::parse::ExPointIncreaseScope::SelfOnly,
        })
    );
    assert_eq!(
        parse(579023, "ExPointIncrChange", &["1".into(), "0".into()]),
        Some(ParsedConditionKind::ExPointIncrChange {
            threshold: 1,
            kind: 0,
            scope: crate::engine::skill::condition::parse::ExPointIncreaseScope::OtherAlly,
        })
    );
}

#[test]
fn conduit_cost_uses_its_exact_activation_subscription() {
    assert_eq!(
        parse(788210, "PerDeviceCurrCost", &["1".into()]),
        Some(ParsedConditionKind::PerConduitCurrentCost { threshold: 1 })
    );
    let definition = find_key(788210, "PerDeviceCurrCost").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::ConduitActivated,
            phase: None,
        }
    );
    assert_eq!(definition.publication, PublicationPhase::AfterPublish);
    assert_eq!(definition.reaction_timing, ReactionTiming::Immediate);
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    assert_eq!(definition.reaction_frame_scope, ReactionFrameScope::Causing);
}

#[test]
fn conduit_meter_and_group_conditions_keep_their_setup_stages() {
    assert_eq!(
        parse(787105, "DeviceExPoint", &["1".into(), "100".into()]),
        Some(ParsedConditionKind::ConduitExPoint {
            compare_code: 1,
            threshold: 100,
        })
    );
    assert_eq!(
        parse(794103, "DeviceSkillIndex", &["3".into()]),
        Some(ParsedConditionKind::ConduitSkillGroup { group: 3 })
    );
    assert_eq!(
        find_key(787105, "DeviceExPoint").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::AfterRoundStart,
            priority: 0,
        })
    );
    for key in [(787103, "DeviceExPoint"), (794103, "DeviceSkillIndex")] {
        assert_eq!(
            find_key(key.0, key.1).map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::RoundStart,
                priority: 1,
            })
        );
    }
}

#[test]
fn actor_post_hit_filter_publishes_before_team_observers() {
    let definition = find_key(34210, "UseSkillEffectTag").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::HitPassives),
        }
    );
    assert_eq!(definition.publication, PublicationPhase::BeforePublish);
}

#[test]
fn post_hit_specific_skill_is_a_filter_with_an_event_dependency() {
    let definition = find_key(66210, "UseSpecificSkill").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
}

#[test]
fn skill_target_count_is_a_filter_with_an_event_dependency() {
    let definition = find_key(500210, "SkillType").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
}

#[test]
fn child_buff_allocation_is_owned_by_the_exact_condition_route() {
    assert_eq!(
        find_key(662208, "ActiveUseSkillId").map(|definition| definition.consequence),
        Some(ConsequencePolicy::ChildBuffGrant)
    );
    assert_eq!(
        find_key(502208, "ActiveUseSkill").map(|definition| definition.consequence),
        Some(ConsequencePolicy::Default)
    );
    assert_eq!(
        find_key(502212, "ActiveUseSkill").map(|definition| definition.consequence),
        Some(ConsequencePolicy::NormalBuffGrant)
    );
    assert_eq!(
        find_key(662208, "ActiveUseSkillId").map(|definition| definition.behavior_target_source),
        Some(BehaviorTargetSource::ActiveSkillTargets)
    );
}

#[test]
fn hurt_kind_opcodes_keep_exact_attacker_type_predicates() {
    assert_eq!(
        parse(20202, "HurtReal", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
    );
    assert_eq!(
        find_key(20202, "HurtReal").and_then(|definition| definition.attack_modifier_side),
        Some(AttackModifierSide::IncomingTarget)
    );
    assert_eq!(
        parse(20204, "HurtReal", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
    );
    assert_eq!(
        find_key(20204, "HurtReal").and_then(|definition| definition.attack_modifier_side),
        Some(AttackModifierSide::IncomingTarget)
    );
    assert_eq!(
        parse(20209, "HurtReal", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
    );
    assert_eq!(
        parse(21209, "HurtMagic", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
    );
    assert_eq!(
        parse(21204, "HurtMagic", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
    );
    assert_eq!(
        find_key(21204, "HurtMagic").and_then(|definition| definition.attack_modifier_side),
        Some(AttackModifierSide::IncomingTarget)
    );
    assert_eq!(
        parse(538203, "EntityHurtMagic", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
    );
    assert_eq!(
        parse(540203, "EntityHurtReal", &[]),
        Some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
    );
    assert!(find_key(20210, "HurtReal").is_none());
    assert!(find_key(20201, "HurtReal").is_none());
    assert!(find_key(21210, "HurtMagic").is_none());
    assert!(find_key(538204, "EntityHurtMagic").is_none());
    assert!(find_key(540204, "EntityHurtReal").is_none());
}

#[test]
fn hero_damage_type_opcodes_keep_exact_battle_start_source_predicates() {
    assert_eq!(
        parse(36021, "HeroReal", &[]),
        Some(ParsedConditionKind::SourceDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
    );
    assert_eq!(
        parse(37021, "HeroMagic", &[]),
        Some(ParsedConditionKind::SourceDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
    );
    for key in [
        DefinitionKey::new(36021, "HeroReal"),
        DefinitionKey::new(37021, "HeroMagic"),
    ] {
        assert!(matches!(
            find_key(key.opcode, key.type_name).map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::BattleStart,
                priority: 0,
            })
        ));
    }
}

#[test]
fn other_ally_damage_type_condition_keeps_its_configured_cap() {
    assert_eq!(
        parse(
            573002,
            "PerTeamOtherEntityDmgType",
            &["2".into(), "2".into()],
        ),
        Some(ParsedConditionKind::OtherAllyDamageTypeCount {
            damage_type: crate::engine::skill::target::EntityDamageType::Mental,
            max_count: 2,
        })
    );
}

#[test]
fn force_field_condition_keeps_its_exact_psychube_route() {
    assert_eq!(
        parse(701210, "HasMasterHalo", &[]),
        Some(ParsedConditionKind::MasterHalo)
    );
    assert!(find_key(701211, "HasMasterHalo").is_none());
}

#[test]
fn loop_chain_thresholds_keep_their_three_exact_events() {
    for (opcode, event) in [
        (535214, EventKind::TargetAttacked),
        (535215, EventKind::AllyAction),
        (535303, EventKind::RoundEndEntitySettlement),
    ] {
        assert_eq!(
            find_key(opcode, "TypeIdBuffCountMoreThan").map(|definition| definition.role),
            Some(ConditionRole::Trigger { event, phase: None })
        );
    }
    assert!(find_key(535216, "TypeIdBuffCountMoreThan").is_none());
}

#[test]
fn incoming_attack_modifier_conditions_keep_their_exact_side() {
    for (opcode, type_name) in [
        (18202, "HasBuff"),
        (19204, "HasBuffId"),
        (57204, "NoBuffId"),
        (25204, "UseExSkill"),
        (33204, "HurtRestraint"),
        (47204, "HurtNotRestraint"),
        (1204, "LifeLess"),
    ] {
        assert_eq!(
            find_key(opcode, type_name).and_then(|definition| definition.attack_modifier_side),
            Some(AttackModifierSide::IncomingTarget)
        );
    }
    assert_eq!(
        find_key(33201, "HurtRestraint").and_then(|definition| definition.attack_modifier_side),
        None
    );
    assert_eq!(
        find_key(18203, "HasBuff").and_then(|definition| definition.attack_modifier_side),
        None
    );
    assert_eq!(
        find_key(19201, "HasBuffId").and_then(|definition| definition.attack_modifier_side),
        None
    );
    let nested = ParsedCondition {
        opcode: 0,
        type_name: "Any".into(),
        kind: ParsedConditionKind::Any(vec![vec![ParsedCondition {
            opcode: 18202,
            type_name: "HasBuff".into(),
            ..ParsedCondition::always()
        }]]),
        raw_args: Vec::new(),
    };
    assert_eq!(
        attack_modifier_side(&[nested]),
        Some(AttackModifierSide::IncomingTarget)
    );
}

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
            compare: super::super::parse::ConditionCompare::GreaterThanOrEqual,
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
            compare: super::super::parse::ConditionCompare::GreaterThanOrEqual,
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
            compare: super::super::parse::ConditionCompare::LessThanOrEqual,
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
            scope: super::super::parse::EntityCountScope::AliveEnemiesIncludeSp,
            compare: super::super::parse::ConditionCompare::Equal,
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
            compare: super::super::parse::ConditionCompare::GreaterThan,
            threshold: 500,
        })
    );
    assert!(find_key(2204, "LifeMore").is_none());
}

#[test]
fn round_end_life_more_gate_uses_a_strict_threshold() {
    assert_eq!(
        parse(2301, "LifeMore", &["900".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::parse::ConditionCompare::GreaterThan,
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
            compare: super::super::parse::ConditionCompare::GreaterThan,
            threshold: 500,
        })
    );
}

#[test]
fn incoming_life_less_gate_keeps_its_exact_modifier_route() {
    assert_eq!(
        parse(1204, "LifeLess", &["500".into()]),
        Some(ParsedConditionKind::HpPermille {
            compare: super::super::parse::ConditionCompare::LessThan,
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
            compare: super::super::parse::ConditionCompare::LessThanOrEqual,
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
fn other_ally_extra_action_keeps_its_exact_route() {
    assert_eq!(
        parse(403212, "SkillExtraType", &["1".into()]),
        Some(ParsedConditionKind::ExtraAction {
            mode: super::super::extra::ExtraActionConditionMode::OtherAllyAction,
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
            compare: super::super::parse::ConditionCompare::GreaterThanOrEqual,
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
            compare: super::super::parse::ConditionCompare::GreaterThanOrEqual,
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
            compare: super::super::parse::ConditionCompare::LessThanOrEqual,
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
fn only_the_proven_enter_fight_key_reactivates_after_transform() {
    assert_eq!(
        find_key(5, "EnterFight").unwrap().reactivation_events,
        &[EventKind::EntityTransformed]
    );
    assert!(
        find_key(595002, "TargetIncludeHero")
            .unwrap()
            .reactivation_events
            .is_empty()
    );
}

#[test]
fn target_career_selects_matching_behavior_targets() {
    let condition = ParsedCondition {
        opcode: 16210,
        type_name: "TargetCareer".into(),
        kind: ParsedConditionKind::TargetCareer(vec![3]),
        raw_args: vec!["3".into()],
    };

    assert!(conditions_filter_behavior_targets(&[condition]));
    assert_eq!(
        find_key(16210, "TargetCareer").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
    assert!(conditions_filter_behavior_targets(&[ParsedCondition {
        opcode: 16002,
        type_name: "TargetCareer".into(),
        kind: ParsedConditionKind::TargetCareer(vec![3]),
        raw_args: vec!["3".into()],
    }]));
    assert!(conditions_filter_behavior_targets(&[ParsedCondition {
        opcode: 16210,
        type_name: "TargetCareer".into(),
        kind: ParsedConditionKind::TargetCareer(vec![3]),
        raw_args: vec!["3".into()],
    }]));
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
fn round_end_buff_id_gate_keeps_its_exact_key() {
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
            event: EventKind::RoundEnd,
            phase: None,
        })
    );
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
            scope: super::super::parse::EntityCountScope::AliveOtherTeammates,
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
fn player_buff_gate_keeps_team_presence_and_exact_buff_identity() {
    assert_eq!(
        parse(
            750101,
            "PlayerHasBuff",
            &["2".into(), "0".into(), "109320002".into()],
        ),
        Some(ParsedConditionKind::TeamBuffPresence {
            team: 2,
            present: false,
            buff_id: 109320002,
        })
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
