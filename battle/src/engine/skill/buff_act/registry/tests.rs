use std::collections::HashSet;

use super::*;

#[test]
fn registry_requires_exact_id_and_type() {
    assert_eq!(
        find(1075, "CardLimitAdd").unwrap().kind,
        BuffActKind::CardLimitAdd
    );
    assert!(find(1075, "CardNotCalSize").is_none());
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
        destination(951, "CardNotCalSize", &[]),
        Some(BuffActDestination::StateConsumer)
    );
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
fn reflect_frame_is_owned_by_holder_and_marks_attacker() {
    let definition = find(303, "Rebound").unwrap();

    assert_eq!(definition.runtime.frame_source, RuntimeFrameSource::Owner);
    assert_eq!(
        definition.runtime.marker,
        Some(RuntimeMarker {
            position: RuntimeMarkerPosition::BeforeChanges,
            target: RuntimeMarkerTarget::EventSource,
        })
    );
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
