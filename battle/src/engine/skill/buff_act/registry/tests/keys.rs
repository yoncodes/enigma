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
            .markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Injuryabsorb as i32]
    );
    assert_eq!(
        find(717, "HealCritFix").unwrap().kind,
        BuffActKind::HealCritFix
    );
    assert_eq!(
        destination(717, "HealCritFix", &[500]),
        Some(BuffActDestination::StateConsumer)
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
    let source_attribute = find(1152, "AttrFromEntityNoLayer").unwrap();
    assert_eq!(
        source_attribute.state.initial,
        Some(BuffActInitialStateRule::SourceAttackAttribute)
    );
    assert!(
        source_attribute
            .wire
            .unwrap()
            .projects_initial_state_on_refresh
    );
    let device_bank = find(1149, "DeviceExPointOverflowBank").unwrap();
    assert_eq!(device_bank.kind, BuffActKind::DeviceExPointOverflowBank);
    assert_eq!(
        runtime_publication(
            1149,
            "DeviceExPointOverflowBank",
            EventKind::ExPointOverflow,
        ),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        runtime_publication(1149, "DeviceExPointOverflowBank", EventKind::ExPointChanged,),
        PublicationPhase::AfterPublish
    );
    assert!(has_destination(1149, "DeviceExPointOverflowBank", &[50]));
    assert!(!has_destination(1149, "DeviceExPointOverflowBank", &[0]));
    let device_bank_wire = device_bank.wire.unwrap();
    assert_eq!(
        device_bank_wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert_eq!(
        device_bank.state.initial,
        Some(BuffActInitialStateRule::ZeroInteger)
    );
    assert!(!device_bank_wire.initial_state_marker);
    let fake_hp = find(1150, "LostHpToFakeHp").expect("LostHpToFakeHp must be registered");
    assert_eq!(fake_hp.kind, BuffActKind::LostHpToFakeHp);
    assert_eq!(
        destination(1150, "LostHpToFakeHp", &[102, 4_000]),
        Some(BuffActDestination::HpLossState)
    );
    assert!(!fake_hp.runtime.effect_time_subscription);
    assert!(fake_hp.runtime.handler.is_none());
    assert!(fake_hp.state.hp_loss.is_some());
    assert!(has_destination(1150, "LostHpToFakeHp", &[102, 4_000]));
    assert!(!has_destination(1150, "LostHpToFakeHp", &[101, 4_000]));
    assert!(!has_destination(1150, "LostHpToFakeHp", &[102, 0]));
    assert!(
        !fake_hp
            .wire
            .expect("LostHpToFakeHp has wire metadata")
            .initial_state_marker
    );
    assert_eq!(
        find(728, "ChangeRemoveBuffUseSkillParam").unwrap().kind,
        BuffActKind::ChangeRemoveBuffUseSkillParam
    );
    assert_eq!(
        destination(728, "ChangeRemoveBuffUseSkillParam", &[1, 1]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(
        destination(728, "ChangeRemoveBuffUseSkillParam", &[1, 2]),
        None
    );
    assert!(find(728, "RemoveBuffUseSkill").is_none());
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
    for event in [EventKind::SkillEffectStarted, EventKind::SkillCast] {
        assert_eq!(
            runtime_publication(748, "UseDamageSkillAddToTarget", event),
            PublicationPhase::AfterPublish
        );
    }
    assert!(!subscribes_to_event(
        748,
        "UseDamageSkillAddToTarget",
        201,
        EventKind::SkillAction
    ));
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
            (SetupStage::OpeningBuffGate, 0),
            (SetupStage::RoundTransitionStart, 3),
        ]
    );
    assert_eq!(
        glow.setup_frame(SetupStage::BattleStart, 0),
        (SetupFrameScope::IndependentStep, 1)
    );
    assert_eq!(
        glow.setup_frame(SetupStage::OpeningBuffGate, 0),
        (SetupFrameScope::MechanicFrame, 0)
    );
    assert_eq!(
        glow.setup_frame(SetupStage::RoundTransitionStart, 3),
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
fn registered_handlers_have_one_execution_path_and_a_capability() {
    for definition in definitions() {
        let runtime_handlers = [
            definition.runtime.handler.is_some(),
            definition.runtime.scoped_handler.is_some(),
            definition.runtime.fallible_scoped_handler.is_some(),
        ]
        .into_iter()
        .filter(|registered| *registered)
        .count();
        assert!(
            runtime_handlers <= 1,
            "{:?} has multiple runtime handlers",
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
            || definition.runtime.fallible_scoped_handler.is_some()
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
