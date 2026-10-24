use super::*;

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
    let definition = find_key(727100, "RoundAfter").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 100,
        }
    );
    assert_eq!(
        definition.opening_owner_eligibility,
        OpeningOwnerEligibility::BothSides
    );
    assert_eq!(
        opening_owner_eligibility(727100, "RoundAfter"),
        Some(OpeningOwnerEligibility::BothSides)
    );
    assert_eq!(
        find_key(101, "None").map(|definition| definition.opening_owner_eligibility),
        Some(OpeningOwnerEligibility::PlayerSide)
    );
    assert_eq!(
        opening_owner_eligibility(101, "None"),
        Some(OpeningOwnerEligibility::PlayerSide)
    );
    assert!(find_key(727100, "None").is_none());
    assert_eq!(opening_owner_eligibility(727100, "None"), None);
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
    assert_eq!(
        find_key(812, "Dead").map(|definition| definition.reaction_observer_scope),
        Some(ReactionObserverScope::ConditionTarget)
    );

    let death = find_key(8, "Dead").unwrap();
    assert_eq!(
        death.reaction_observer_scope,
        ReactionObserverScope::ConditionTarget
    );
    assert_eq!(death.reaction_frame_source, ReactionFrameSource::Side);
    assert_eq!(
        death.reaction_frame_target,
        ReactionFrameTarget::EventTarget
    );
    assert_eq!(
        death.reaction_timing.resolve(Default::default()),
        ReactionTiming::Immediate
    );
}

#[test]
fn exact_self_exit_aliases_follow_the_exiting_owner() {
    for opcode in [648003, 648009] {
        assert_eq!(
            parse(opcode, "SelfExit", &[]),
            Some(ParsedConditionKind::EntityDead)
        );
        let definition = find_key(opcode, "SelfExit").unwrap();
        assert_eq!(
            definition.role,
            ConditionRole::Trigger {
                event: EventKind::EntityDied,
                phase: None,
            }
        );
        assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
        assert_eq!(
            definition.reaction_observer_scope,
            ReactionObserverScope::EventTarget
        );
        assert!(parse(opcode, "Dead", &[]).is_none());
        assert!(parse(opcode, "SelfExit", &["1".into()]).is_none());
    }
}

#[test]
fn exact_alive_team_count_rechecks_on_entity_death() {
    assert!(matches!(
        parse(616012, "TeammateAliveNumNoSp", &["0".into()]),
        Some(ParsedConditionKind::EntityCount {
            scope: super::super::super::parse::EntityCountScope::AliveTeammatesNoSp,
            compare: super::super::super::parse::ConditionCompare::Equal,
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
        find_key(566, "PowerUseAddBuff")
            .map(|definition| definition.reaction_timing.resolve(Default::default())),
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
    for (opcode, scope, frame_scope, frame_target) in [
        (
            788210,
            crate::engine::skill::condition::parse::ConduitActivationScope::Owner,
            ReactionFrameScope::Causing,
            ReactionFrameTarget::Owner,
        ),
        (
            788212,
            crate::engine::skill::condition::parse::ConduitActivationScope::Team,
            ReactionFrameScope::Subscriber,
            ReactionFrameTarget::CausingFrame,
        ),
    ] {
        assert_eq!(
            parse(opcode, "PerDeviceCurrCost", &["1".into()]),
            Some(ParsedConditionKind::PerConduitCurrentCost {
                threshold: 1,
                scope,
            })
        );
        let definition = find_key(opcode, "PerDeviceCurrCost").unwrap();
        assert_eq!(
            definition.role,
            ConditionRole::Trigger {
                event: EventKind::ConduitActivated,
                phase: None,
            }
        );
        assert_eq!(definition.publication, PublicationPhase::AfterPublish);
        assert_eq!(
            definition.reaction_timing.resolve(Default::default()),
            ReactionTiming::Immediate
        );
        assert_eq!(definition.reaction_frame_target, frame_target);
        assert_eq!(definition.reaction_frame_scope, frame_scope);
    }
    assert_eq!(parse(788212, "PerDeviceCurrCost", &[]), None);
    assert!(find_key(788211, "PerDeviceCurrCost").is_none());
}

#[test]
fn use_device_skill_uses_its_exact_activation_subscription() {
    assert_eq!(
        parse(792208, "UseDeviceSkill", &[]),
        Some(ParsedConditionKind::None(NoneMode::Unconditional))
    );
    assert_eq!(parse(792208, "UseDeviceSkill", &["1".into()]), None);
    let definition = find_key(792208, "UseDeviceSkill").unwrap();
    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::ConduitActivated,
            phase: None,
        }
    );
    assert_eq!(definition.reaction_frame_target, ReactionFrameTarget::Owner);
    assert_eq!(definition.reaction_frame_scope, ReactionFrameScope::Causing);
    assert!(find_key(792208, "PerDeviceCurrCost").is_none());
}

#[test]
fn conduit_counter_uses_its_exact_predicate_route() {
    assert_eq!(
        parse(
            786203,
            "PerDeviceCounter",
            &["1".into(), "1".into(), "20".into()],
        ),
        Some(ParsedConditionKind::PerConduitCounter {
            kind: ConduitCounterKind::EnergyAccumulation,
            divisor: 1,
            max_count: 20,
        })
    );
    assert_eq!(
        parse(
            786203,
            "PerDeviceCounter",
            &["2".into(), "1".into(), "20".into()],
        ),
        Some(ParsedConditionKind::PerConduitCounter {
            kind: ConduitCounterKind::Activation,
            divisor: 1,
            max_count: 20,
        })
    );
    for args in [
        vec![],
        vec!["1".into(), "1".into()],
        vec!["3".into(), "1".into(), "20".into()],
        vec!["1".into(), "0".into(), "20".into()],
        vec!["1".into(), "1".into(), "0".into()],
    ] {
        assert_eq!(parse(786203, "PerDeviceCounter", &args), None);
    }
    let definition = find_key(786203, "PerDeviceCounter").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::ConduitActivated]);
    assert!(find_key(786303, "PerDeviceCounter").is_none());
    assert!(find_key(786203, "PerDeviceCurrCost").is_none());
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
fn actor_post_hit_filter_runs_after_the_source_skill() {
    let definition = find_key(34210, "UseSkillEffectTag").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::HitPassives),
        }
    );
    assert_eq!(definition.publication, PublicationPhase::BeforePublish);
    assert_eq!(
        definition.reaction_timing.resolve(Default::default()),
        ReactionTiming::AfterSkill
    );
    assert_eq!(
        definition.reaction_frame_target,
        ReactionFrameTarget::CausingFrame
    );
}

#[test]
fn post_hit_specific_skill_is_a_filter_with_an_event_dependency() {
    let definition = find_key(66210, "UseSpecificSkill").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
}

#[test]
fn received_hit_skill_rank_is_an_exact_target_attacked_filter() {
    let definition = find_key(66209, "UseSpecificSkill").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::TargetAttacked]);
    assert_eq!(
        parse(66209, "UseSpecificSkill", &["5".into(), "3".into()]),
        Some(ParsedConditionKind::ReceivedSpecificSkill { group: 5, rank: 3 })
    );
}

#[test]
fn round_start_buff_type_threshold_keeps_its_exact_late_lane() {
    assert_eq!(
        parse(
            535104,
            "TypeIdBuffCountMoreThan",
            &["4150001".into(), "30".into()]
        ),
        Some(ParsedConditionKind::BuffTypeCount {
            type_ids: vec![4_150_001],
            compare: crate::engine::skill::condition::parse::ConditionCompare::GreaterThanOrEqual,
            threshold: 30,
        })
    );
    assert_eq!(
        find_key(535104, "TypeIdBuffCountMoreThan").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartLate,
            priority: 0,
        })
    );
}

#[test]
fn round_start_allies_buff_type_gate_keeps_its_exact_lane() {
    assert_eq!(
        parse(
            550102,
            "AllInBuffByTypeId",
            &["117373041".into(), "1".into()]
        ),
        Some(ParsedConditionKind::AllTargetBuffTypeCount {
            type_ids: vec![117373041],
            threshold: 1,
        })
    );
    assert_eq!(
        find_key(550102, "AllInBuffByTypeId").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 102,
        })
    );
}

#[test]
fn round_start_team_buff_layers_keep_their_exact_payload_order() {
    for (opcode, priority) in [(513101, 101), (513102, 102)] {
        assert_eq!(
            parse(
                opcode,
                "SelfTeamHasBuffTypeLayerMoreThan",
                &["6".into(), "117373041".into()]
            ),
            Some(ParsedConditionKind::BuffTypeCount {
                type_ids: vec![117373041],
                compare:
                    crate::engine::skill::condition::parse::ConditionCompare::GreaterThanOrEqual,
                threshold: 6,
            })
        );
        assert_eq!(
            find_key(opcode, "SelfTeamHasBuffTypeLayerMoreThan").map(|definition| definition.role),
            Some(ConditionRole::Setup {
                stage: SetupStage::RoundStartCondition,
                priority,
            })
        );
    }
}

#[test]
fn skill_target_count_is_a_filter_with_an_event_dependency() {
    let definition = find_key(500210, "SkillType").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
}

#[test]
fn immediate_skill_type_gate_keeps_its_exact_identity() {
    let definition = find_key(500203, "SkillType").unwrap();

    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::SkillAction]);
    assert_eq!(
        parse(500203, "SkillType", &["1".into()]),
        Some(ParsedConditionKind::ActiveSkillType(1))
    );
    assert!(find_key(500203, "UseSkillEffectTag").is_none());
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
fn active_ally_reactions_inherit_the_causing_frame_target() {
    let definition = find_key(502212, "ActiveUseSkill").unwrap();

    assert_eq!(
        definition.role,
        ConditionRole::Trigger {
            event: EventKind::AllyAction,
            phase: None,
        }
    );
    assert_eq!(
        definition.reaction_frame_target,
        ReactionFrameTarget::CausingFrame
    );
    assert_eq!(definition.consequence, ConsequencePolicy::NormalBuffGrant);
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
        (1007204, "FromBuffAndToBuff"),
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
