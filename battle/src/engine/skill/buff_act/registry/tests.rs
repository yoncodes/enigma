use std::collections::HashSet;

use super::*;

#[test]
fn registry_requires_exact_id_and_type() {
    assert_eq!(
        find(1075, "CardLimitAdd").unwrap().kind,
        BuffActKind::CardLimitAdd
    );
    assert!(find(1075, "CardNotCalSize").is_none());
    let leech_reduction = find(716, "InjuryAbsorb").unwrap();
    assert_eq!(leech_reduction.kind, BuffActKind::InjuryAbsorb);
    assert_eq!(
        destination(716, "InjuryAbsorb", &[300]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(716, "InjuryAbsorb", &[1001]));
    assert_eq!(
        leech_reduction
            .wire
            .unwrap()
            .markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Injuryabsorb as i32]
    );
    assert_eq!(
        find(1137, "EntityExSkillNotCalSize").unwrap().kind,
        BuffActKind::EntityExSkillNotCalSize
    );
    assert_eq!(
        destination(1137, "EntityExSkillNotCalSize", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(find(1137, "CardNotCalSize").is_none());
    assert_eq!(
        destination(951, "CardNotCalSize", &[1]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(destination(951, "CardNotCalSize", &[]), None);
    assert_eq!(
        find(1028, "RealDamageKill").unwrap().kind,
        BuffActKind::RealDamageKill
    );
    assert_eq!(
        find(922, "AttrAndLayerAttr").unwrap().kind,
        BuffActKind::AttrAndLayerAttr
    );
    assert_eq!(
        find(752, "AttrByDmgType").unwrap().kind,
        BuffActKind::AttrByDamageType
    );
    assert!(has_destination(752, "AttrByDmgType", &[2, 203, 200]));
    assert!(!has_destination(752, "AttrByDmgType", &[3, 203, 200]));
    assert!(find(752, "AttrByHeroId").is_none());
    assert!(find(922, "FixAttrBySubBuffLayer").is_none());
    assert!(has_destination(865, "AddPassiveSkills", &[31260183]));
    assert!(!has_destination(865, "AddPassiveSkills", &[0]));
    assert!(has_destination(933, "SubBuff", &[31260201]));
    assert!(!has_destination(933, "SubBuff", &[0]));
    assert!(find(1028, "AddToTarget").is_none());
    assert!(find(999, "RealDamageKill").is_none());
    assert!(reserves_trigger_child_uid(DefinitionKey::new(
        1053,
        "AttrByHeatScale"
    )));
    assert_eq!(
        runtime_event(1051, "CrystalAddBuff", 208),
        Some(EventKind::SkillAction)
    );
    assert_eq!(
        runtime_phase(1051, "CrystalAddBuff", 208, EventKind::SkillAction),
        Some(crate::engine::skill::action::SkillPhase::AfterDamage)
    );
    assert_eq!(
        runtime_event(928, "AddToTarget", 908),
        Some(EventKind::SkillAction)
    );
    assert_eq!(
        find(928, "AddToTarget").unwrap().kind,
        BuffActKind::AddToAttackTargets
    );
    assert_eq!(
        runtime_phase(928, "AddToTarget", 908, EventKind::SkillAction),
        Some(crate::engine::skill::action::SkillPhase::AfterDamage)
    );
    assert!(!reserves_trigger_child_uid(DefinitionKey::new(
        1004,
        "AddAttrBySpecialCount"
    )));
    assert_eq!(
        find(803, "Poison").unwrap().state.read_timing,
        StatReadTiming::OnGrant
    );
    assert_eq!(
        find(861, "FixTempAttrByBuffLayer")
            .unwrap()
            .state
            .read_timing,
        StatReadTiming::OnTrigger
    );
    assert_eq!(
        find(860, "MustCritAndFixTempAttr")
            .unwrap()
            .state
            .read_timing,
        StatReadTiming::OnTrigger
    );
    assert_eq!(
        find(726, "Burn").unwrap().state.read_timing,
        StatReadTiming::OnTrigger
    );
    assert_eq!(
        find(203, "Dot").unwrap().state.read_timing,
        StatReadTiming::ByArguments
    );
    for event in [EventKind::SkillAction, EventKind::SkillCast] {
        assert_eq!(
            runtime_publication(748, "UseDamageSkillAddToTarget", event),
            if event == EventKind::SkillAction {
                PublicationPhase::BeforePublish
            } else {
                PublicationPhase::AfterPublish
            }
        );
    }
    for (act_id, act_type) in [(100, "Attr"), (834, "EachChangeAttr")] {
        assert_eq!(
            runtime_publication(act_id, act_type, EventKind::BuffAdded),
            PublicationPhase::BeforePublish
        );
    }
    let card_record = find(929, "AddCardRecordByRound").unwrap();
    assert_eq!(
        card_record.runtime.event_override,
        Some(EventKind::ActionQueueCommitted)
    );
    assert_eq!(
        card_record.runtime.publication,
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        card_record.runtime.frame_scope,
        RuntimeFrameScope::IndependentEvent
    );
    let emitter = find(875, "EmitterTag").unwrap();
    assert_eq!(
        emitter.runtime.events,
        &[
            EventKind::ActionQueueCommitted,
            EventKind::ImpromptuResolved,
        ]
    );
    assert_eq!(emitter.runtime.frame_scope, RuntimeFrameScope::CausingFrame);
    assert_eq!(emitter.setup.routes, &[(SetupStage::BattleStart, 0)]);
    assert_eq!(
        emitter.setup_frame(SetupStage::BattleStart, 0),
        (SetupFrameScope::IndependentStep, 0)
    );
    let emitter_threshold = find(893, "EmitterEnergyAddBuff").unwrap();
    assert_eq!(
        emitter_threshold.runtime.event_override,
        Some(EventKind::PlayerActionsResolved)
    );
    assert_eq!(
        emitter_threshold.runtime.frame_scope,
        RuntimeFrameScope::IndependentEvent
    );
    let blood = find(953, "BloodPoolTag").unwrap();
    assert_eq!(
        blood.runtime.events,
        &[EventKind::HpLost, EventKind::GaugeChanged]
    );
    assert_eq!(
        runtime_publication(953, "BloodPoolTag", EventKind::HpLost),
        PublicationPhase::AfterPublish
    );
    assert_eq!(
        runtime_publication(953, "BloodPoolTag", EventKind::GaugeChanged),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        blood.setup.routes,
        &[
            (SetupStage::BattleStart, 0),
            (SetupStage::Unconditional, 0),
            (SetupStage::RoundStart, -1),
        ]
    );
    assert_eq!(
        blood.setup_frame(SetupStage::BattleStart, 0),
        (SetupFrameScope::IndependentStep, 1)
    );
    assert_eq!(
        blood.setup_frame(SetupStage::Unconditional, 0),
        (SetupFrameScope::IndependentStep, 0)
    );
    assert_eq!(
        blood.setup_frame(SetupStage::RoundStart, -1),
        (SetupFrameScope::IndependentStep, 0)
    );
    let glow = find(1052, "HeatScaleTag").unwrap();
    assert_eq!(
        glow.runtime.events,
        &[
            EventKind::BuffAdded,
            EventKind::BuffChanged,
            EventKind::GaugeChanged,
        ]
    );
    assert_eq!(
        runtime_publication(1052, "HeatScaleTag", EventKind::BuffAdded),
        PublicationPhase::AfterPublish
    );
    assert_eq!(
        runtime_publication(1052, "HeatScaleTag", EventKind::GaugeChanged),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        glow.setup.routes,
        &[
            (SetupStage::BattleStart, 0),
            (SetupStage::BuffGate, 0),
            (SetupStage::RoundStart, 3),
        ]
    );
    assert_eq!(
        glow.setup_frame(SetupStage::BattleStart, 0),
        (SetupFrameScope::IndependentStep, 1)
    );
    assert_eq!(
        glow.setup_frame(SetupStage::BuffGate, 0),
        (SetupFrameScope::MechanicFrame, 0)
    );
    assert_eq!(
        glow.setup_frame(SetupStage::RoundStart, 3),
        (SetupFrameScope::RootMechanicFrame, 0)
    );
    assert!(
        !find(501, "Shield")
            .unwrap()
            .runtime
            .effect_time_subscription
    );
}

#[test]
fn damage_cap_is_an_exact_static_consumer_with_its_captured_marker() {
    let definition = find(510, "DamageNotMoreThan").unwrap();

    assert_eq!(definition.kind, BuffActKind::DamageNotMoreThan);
    assert!(definition.state.consumer);
    assert!(has_destination(510, "DamageNotMoreThan", &[300]));
    assert!(!has_destination(510, "DamageNotMoreThan", &[]));
    assert!(find(510, "FixedHurt").is_none());
    assert_eq!(
        super::super::wire::find(510, "DamageNotMoreThan")
            .unwrap()
            .markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Damagenotmorethan as i32]
    );
}

#[test]
fn fixed_damage_accepts_zero_without_collapsing_identity() {
    let definition = find(511, "FixedHurt").unwrap();

    assert_eq!(definition.kind, BuffActKind::FixedHurt);
    assert!(definition.state.consumer);
    assert!(has_destination(511, "FixedHurt", &[0]));
    assert!(has_destination(511, "FixedHurt", &[1]));
    assert!(!has_destination(511, "FixedHurt", &[-1]));
    assert!(find(511, "DamageNotMoreThan").is_none());
}

#[test]
fn hp_loss_floor_is_an_exact_static_consumer() {
    let definition = find(1008, "BanLostLife").unwrap();

    assert_eq!(definition.kind, BuffActKind::BanLostLife);
    assert!(definition.state.consumer);
    assert!(has_destination(1008, "BanLostLife", &[150]));
    assert!(!has_destination(1008, "BanLostLife", &[]));
    assert!(!has_destination(1008, "BanLostLife", &[160]));
    assert!(!has_destination(1008, "BanLostLife", &[500]));
    assert!(find(1008, "DamageNotMoreThan").is_none());
}

#[test]
fn missing_hp_healing_is_an_exact_static_consumer() {
    let definition = find(1011, "CureUpByLostHp").unwrap();

    assert_eq!(definition.kind, BuffActKind::CureUpByLostHp);
    assert!(definition.state.consumer);
    assert!(has_destination(1011, "CureUpByLostHp", &[200, 75, 8, 100]));
    assert!(!has_destination(1011, "CureUpByLostHp", &[100, 50, 8, 100]));
    assert!(!has_destination(1011, "CureUpByLostHp", &[]));
    assert!(find(1011, "BanLostLife").is_none());
}

#[test]
fn sentinel_sub_buff_chain_uses_exact_state_consumers() {
    assert!(has_destination(
        932,
        "FixAttrBySubBuffLayer",
        &[31260151, 201, 300, 0]
    ));
    assert!(has_destination(933, "SubBuff", &[31260201]));
    assert!(has_destination(865, "AddPassiveSkills", &[31260181]));

    assert!(!has_destination(
        932,
        "FixAttrBySubBuffLayer",
        &[31260151, 201, 300]
    ));
    assert!(!has_destination(933, "SubBuff", &[-1]));
    assert!(!has_destination(865, "AddPassiveSkills", &[0]));
    assert!(find(932, "FixTempAttrByBuffLayer").is_none());
    assert!(find(933, "AddPassiveSkills").is_none());
    assert!(find(865, "SubBuff").is_none());
}

#[test]
fn burn_damage_fix_is_an_exact_static_consumer_with_its_add_marker() {
    let definition = find(793, "BurnRealHurtFix").unwrap();

    assert_eq!(definition.kind, BuffActKind::BurnRealHurtFix);
    assert!(definition.state.consumer);
    assert!(has_destination(793, "BurnRealHurtFix", &[500, 150]));
    assert!(has_destination(793, "BurnRealHurtFix", &[-1000, 0]));
    assert!(!has_destination(793, "BurnRealHurtFix", &[-1001, 0]));
    assert!(!has_destination(793, "BurnRealHurtFix", &[1001, 0]));
    assert!(!has_destination(793, "BurnRealHurtFix", &[500, 1001]));

    let wire = super::super::wire::find(793, "BurnRealHurtFix").unwrap();
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Realhurtfixwithlimit as i32]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn real_hurt_fix_uses_only_its_captured_add_and_refresh_markers() {
    let wire = super::super::wire::find(519, "RealHurtFix").unwrap();
    let marker = sonettobuf::effect_type_enum::EffectType::Realhurtfix as i32;

    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[marker]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Refresh),
        &[marker]
    );
}

#[test]
fn dot_uses_only_its_captured_add_and_refresh_markers() {
    let wire = super::super::wire::find(202, "Dot").unwrap();
    let marker = sonettobuf::effect_type_enum::EffectType::Dot as i32;

    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[marker]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Refresh),
        &[marker]
    );
}

#[test]
fn lucy_static_combat_rules_keep_distinct_add_markers() {
    for (act_id, act_type, kind, effect_type) in [
        (
            761,
            "IgnoreDodgeSpecSkill",
            BuffActKind::IgnoreDodgeSpecSkill,
            sonettobuf::effect_type_enum::EffectType::Ignoredodgespecskill,
        ),
        (
            763,
            "IgnoreRebound",
            BuffActKind::IgnoreRebound,
            sonettobuf::effect_type_enum::EffectType::Ignorerebound,
        ),
        (
            764,
            "CareerRestraint",
            BuffActKind::CareerRestraint,
            sonettobuf::effect_type_enum::EffectType::Careerrestraint,
        ),
    ] {
        let definition = find(act_id, act_type).unwrap();
        assert_eq!(definition.kind, kind);
        assert_eq!(
            destination(act_id, act_type, &[]),
            Some(BuffActDestination::StateConsumer)
        );
        assert!(!has_destination(act_id, act_type, &[1]));
        assert_eq!(
            definition
                .wire
                .unwrap()
                .markers(super::super::wire::WirePhase::Add),
            &[effect_type as i32]
        );
    }
}

#[test]
fn passive_state_consumers_reject_unsupported_argument_shapes() {
    for (id, kind, valid, invalid) in [
        (794, "ModifyMaxBurnLayers", vec![20], vec![]),
        (865, "AddPassiveSkills", vec![31200221], vec![0]),
        (
            932,
            "FixAttrBySubBuffLayer",
            vec![31260151, 201, 300, 0],
            vec![31260151, 999, 300, 0],
        ),
        (933, "SubBuff", vec![31260201], vec![0]),
        (951, "CardNotCalSize", vec![31340161], vec![]),
        (1137, "EntityExSkillNotCalSize", vec![], vec![1]),
        (
            1053,
            "AttrByHeatScale",
            vec![205, 25, 600_000, 100_000],
            vec![205, 25, 600_000, 0],
        ),
    ] {
        assert!(has_destination(id, kind, &valid), "{id} {kind}");
        assert!(!has_destination(id, kind, &invalid), "{id} {kind}");
    }
}

#[test]
fn layered_attribute_bonus_is_an_exact_static_consumer() {
    let args = [201, 300, 31280114, 75, 4, 206, 0];
    let definition = find(1029, "AddAttrByOtherBuffLayer").unwrap();

    assert_eq!(definition.kind, BuffActKind::AddAttrByOtherBuffLayer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1029, "AddAttrByOtherBuffLayer", &args));
    assert!(!has_destination(
        1029,
        "AddAttrByOtherBuffLayer",
        &args[..6]
    ));
    assert!(find(1036, "AddAttrByOtherBuffLayer").is_some());
}

#[test]
fn layered_attribute_penalty_is_an_exact_static_consumer() {
    let args = [204, -200, 31280114, -75, 4, 206, -400];
    let definition = find(1036, "AddAttrByOtherBuffLayer").unwrap();

    assert_eq!(definition.kind, BuffActKind::AddAttrByOtherBuffLayer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1036, "AddAttrByOtherBuffLayer", &args));
    assert!(!has_destination(
        1036,
        "AddAttrByOtherBuffLayer",
        &args[..6]
    ));
    assert!(find(1029, "AddAttrByOtherBuffLayer").is_some());
}

#[test]
fn field_upgrade_modifier_is_an_exact_static_consumer() {
    let definition = find(1032, "FixElectricUpgrade").unwrap();

    assert_eq!(definition.kind, BuffActKind::FixElectricUpgrade);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1032, "FixElectricUpgrade", &[2, 40, 3]));
    assert!(!has_destination(1032, "FixElectricUpgrade", &[2, 40, 2]));
    assert!(find(1032, "AddAttrByOtherBuffLayer").is_none());
}

#[test]
fn healing_taken_modifier_is_an_exact_static_consumer() {
    let definition = find(601, "Injury").unwrap();

    assert_eq!(definition.kind, BuffActKind::Injury);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(601, "Injury", &[500]));
    assert!(has_destination(601, "Injury", &[-250]));
    assert!(!has_destination(601, "Injury", &[0]));
    assert!(find(601, "InjuryAbsorb").is_none());
}

#[test]
fn registered_exact_keys_are_unique() {
    let definitions = definitions().collect::<Vec<_>>();
    let unique = definitions
        .iter()
        .map(|definition| definition.key)
        .collect::<HashSet<_>>();

    assert_eq!(definitions.len(), unique.len());
}

#[test]
fn runtime_capability_lives_on_the_exact_registry_entry() {
    let definition = find(889, "BeAttackByEmitterDamage").unwrap();

    assert!(definition.runtime.handler.is_some());
    assert!(definition.supports.unwrap()(&[102, 1_000, 5]));
    assert!(!definition.supports.unwrap()(&[102, 1_000, 0]));
    assert!(find(889, "Other").is_none());
}

#[test]
fn reflect_frames_are_owned_by_holder_and_mark_attacker() {
    for definition in [
        find(303, "Rebound").unwrap(),
        find(743, "ReboundBasedOnDamage").unwrap(),
    ] {
        assert_eq!(definition.runtime.frame_source, RuntimeFrameSource::Owner);
        let marker = definition.runtime.marker.unwrap();
        assert_eq!(marker.position, RuntimeMarkerPosition::BeforeChanges);
        assert_eq!(marker.target, RuntimeMarkerTarget::EventSource);
    }

    assert_eq!(
        find(303, "Rebound")
            .unwrap()
            .runtime
            .marker
            .unwrap()
            .effect_type,
        None
    );

    let damage_based = find(743, "ReboundBasedOnDamage").unwrap();
    assert_eq!(damage_based.kind, BuffActKind::ReboundBasedOnDamage);
    assert!(damage_based.supports.unwrap()(&[300, 0, 0]));
    assert!(!damage_based.supports.unwrap()(&[300, 101, 2_000]));
    assert!(!damage_based.supports.unwrap()(&[150, 102, 1_000]));
    let wire = damage_based.wire.unwrap();
    assert!(!wire.has_output());
    assert_eq!(
        damage_based.runtime.marker.unwrap().effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Rebound as i32)
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
    assert!(find(743, "Rebound").is_none());
}

#[test]
fn transaction_capability_lives_on_the_exact_registry_entry() {
    let definition = find(875, "EmitterTag").unwrap();

    assert_eq!(definition.transaction.events, &[EventKind::EurekaChanged]);
    assert!(definition.transaction.handler.is_some());
    assert!(
        !definition
            .runtime
            .events
            .contains(&EventKind::EurekaChanged)
    );
}

#[test]
fn round_start_settlement_keeps_fresh_wound_after_shadow_cloak() {
    assert_eq!(
        runtime_settlement_phase(1042, "Raspberry"),
        RuntimeSettlementPhase::Before
    );
    assert_eq!(
        runtime_settlement_phase(1023, "LostHpAddExtraBloodPoolValue"),
        RuntimeSettlementPhase::After
    );
}

#[test]
fn crystal_attack_grants_run_after_lethal_hit_settlement() {
    assert_eq!(
        runtime_settlement_phase(1051, "CrystalAddBuff"),
        RuntimeSettlementPhase::After
    );
}

#[test]
fn registered_handlers_have_one_execution_path_and_a_capability() {
    for definition in definitions() {
        assert!(
            definition.runtime.handler.is_none() || definition.runtime.scoped_handler.is_none(),
            "{:?} has two runtime handlers",
            definition.key
        );
        assert_eq!(
            definition.transaction.events.is_empty(),
            definition.transaction.handler.is_none(),
            "{:?} must declare transaction events and a handler together",
            definition.key
        );
        assert!(
            definition.setup.routes.is_empty() || definition.setup.handler.is_some(),
            "{:?} has setup routes without a setup handler",
            definition.key
        );
        if definition.runtime.handler.is_some()
            || definition.runtime.scoped_handler.is_some()
            || definition.transaction.handler.is_some()
            || definition.setup.handler.is_some()
        {
            assert!(
                definition.destination().is_some(),
                "{:?} has a handler without semantic ownership",
                definition.key
            );
        }
    }
}

#[test]
fn dot_uses_its_exact_after_hit_route_and_known_argument_shapes() {
    assert_eq!(runtime_event(203, "Dot", 210), Some(EventKind::SkillAction));
    assert_eq!(
        runtime_phase(203, "Dot", 210, EventKind::SkillAction),
        Some(crate::engine::skill::action::SkillPhase::AfterHit)
    );
    assert!(has_destination(203, "Dot", &[0, 102, 1_000]));
    assert!(has_destination(203, "Dot", &[1, 100, 150]));
    assert!(has_destination(203, "Dot", &[1, 101, 200]));
    assert!(!has_destination(203, "Dot", &[0, 101, 200]));
}

#[test]
fn holder_scaled_dot_keeps_its_exact_round_end_route() {
    assert_eq!(runtime_event(202, "Dot", 302), Some(EventKind::RoundEnd));
    assert!(has_destination(202, "Dot", &[1, 100, 30]));
    assert!(find(202, "Dot").is_some());
    assert!(find(202, "DotNoLimit").is_none());
}

#[test]
fn moxie_loss_keeps_its_exact_round_end_route() {
    assert_eq!(
        runtime_event(605, "ExPointDel", 302),
        Some(EventKind::RoundEnd)
    );
    assert!(has_destination(605, "ExPointDel", &[1]));
    assert!(!has_destination(605, "ExPointDel", &[0]));
}

#[test]
fn targeted_support_dispel_keeps_its_exact_skill_cast_route() {
    assert_eq!(
        runtime_event(804, "DisperseByTag", 208),
        Some(EventKind::SkillCast)
    );
    assert_eq!(
        runtime_actor_scope(804, "DisperseByTag"),
        RuntimeActorScope::Team
    );
    assert!(has_destination(804, "DisperseByTag", &[1, 4, 5, 6, 9]));
    assert!(!has_destination(804, "DisperseByTag", &[0, 4, 5, 6, 9]));
}

#[test]
fn layer_gated_passive_keeps_its_exact_static_route() {
    let definition = find(805, "AddPassiveSkillByLayer").unwrap();
    assert!(definition.state.consumer);
    assert!(has_destination(
        805,
        "AddPassiveSkillByLayer",
        &[10, 12110011]
    ));
    assert!(!has_destination(
        805,
        "AddPassiveSkillByLayer",
        &[0, 12110011]
    ));
}

#[test]
fn moxie_reduction_immunity_keeps_its_exact_static_identity() {
    let definition = find(509, "ImmunityExpointChange").unwrap();
    assert_eq!(definition.kind, BuffActKind::MoxieReductionImmunity);
    assert!(definition.state.consumer);
    assert!(has_destination(509, "ImmunityExpointChange", &[]));
    assert!(find(509, "ExPointCantAdd").is_none());
}

#[test]
fn absolute_missing_hp_attributes_keep_their_exact_static_routes() {
    assert_eq!(
        destination(853, "AttrByLostHp", &[10_000_000, 215, 100, 1, 1, 0]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(
        destination(1056, "AttrByLostHp", &[10_000_000, 216, 150, 1, 1, 1]),
        Some(BuffActDestination::StateConsumer)
    );
    for act_id in [853, 1056] {
        let wire = super::super::wire::find(act_id, "AttrByLostHp").unwrap();
        assert_eq!(
            wire.markers(super::super::wire::WirePhase::Add),
            &[sonettobuf::effect_type_enum::EffectType::None as i32]
        );
        assert!(
            wire.markers(super::super::wire::WirePhase::Static)
                .is_empty()
        );
        assert!(
            wire.markers(super::super::wire::WirePhase::Refresh)
                .is_empty()
        );
    }
}

#[test]
fn advanced_cure_owns_its_exact_hit_duration_advance() {
    assert!(owns_duration(849, "AdvancedCure"));
    assert!(!owns_duration(201, "Cure"));
}

#[test]
fn incapacitating_control_buffs_keep_distinct_exact_routes() {
    let dizzy = find(401, "Dizzy").unwrap();
    assert_eq!(dizzy.kind, BuffActKind::Dizzy);
    assert!(dizzy.state.consumer);
    assert!(has_destination(401, "Dizzy", &[]));

    let petrified = find(402, "Petrified").unwrap();
    assert_eq!(petrified.kind, BuffActKind::Petrified);
    assert!(petrified.state.consumer);
    assert_eq!(
        runtime_event(402, "Petrified", 0),
        Some(EventKind::TargetAttacked)
    );
    assert!(has_destination(402, "Petrified", &[]));
    assert!(
        !super::super::wire::find(402, "Petrified")
            .unwrap()
            .has_output()
    );
    assert!(find(402, "Dizzy").is_none());
}
