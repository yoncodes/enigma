use super::*;

#[test]
fn hurt_to_progress_owns_the_pre_damage_hit_route() {
    let definition = find(1148, "HurtToProgress").unwrap();

    assert_eq!(definition.kind, BuffActKind::HurtToProgress);
    assert_eq!(
        runtime_event(1148, "HurtToProgress", 0),
        Some(EventKind::BeAttacked)
    );
    assert_eq!(
        definition.runtime.publication,
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        definition.runtime.frame_scope,
        RuntimeFrameScope::CausingFrame
    );
    let wire = definition.wire.unwrap();
    for phase in [
        super::super::super::wire::WirePhase::Add,
        super::super::super::wire::WirePhase::Static,
        super::super::super::wire::WirePhase::Refresh,
    ] {
        assert_eq!(
            wire.markers(phase),
            &[sonettobuf::effect_type_enum::EffectType::None as i32]
        );
    }
}

#[test]
fn attr_transactions_follow_the_buff_lifecycle_boundary() {
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffAdded),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffChanged),
        PublicationPhase::AfterPublish
    );
    assert_eq!(
        runtime_publication(100, "Attr", EventKind::BuffRemoved),
        PublicationPhase::AfterPublish
    );
}

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
fn card_level_add_keeps_exact_arguments_and_round_start_subscription() {
    let definition = find(701, "CardLevelAdd").expect("CardLevelAdd must be registered exactly");
    assert_eq!(definition.kind, BuffActKind::CardLevelAdd);
    assert_eq!(
        runtime_event(701, "CardLevelAdd", 102),
        Some(EventKind::RoundStart)
    );
    assert!(subscribes_to_event(
        701,
        "CardLevelAdd",
        102,
        EventKind::RoundStart
    ));
    assert!(!subscribes_to_event(
        701,
        "CardLevelAdd",
        102,
        EventKind::RoundEnd
    ));
    assert!(find(701, "CardLevelChange").is_none());
    assert!(find(50011, "CardLevelAdd").is_none());

    let supports = definition
        .supports
        .expect("CardLevelAdd validates arguments");
    for args in [[1], [2], [9]] {
        assert!(supports(&args));
        assert!(has_destination(701, "CardLevelAdd", &args));
    }
    for args in [
        &[][..],
        &[0][..],
        &[3][..],
        &[1, 1][..],
        &[2, 0][..],
        &[9, 1][..],
    ] {
        assert!(!supports(args));
        assert!(!has_destination(701, "CardLevelAdd", args));
    }

    let wire = definition.wire.expect("CardLevelAdd has an add marker");
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Cardleveladd as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn emitter_card_allocation_is_an_exact_state_consumer() {
    let definition = find(879, "EmitterCardAllocateChange").unwrap();
    let args = [1, 300, 1, 2];

    assert_eq!(definition.kind, BuffActKind::EmitterCardAllocateChange);
    assert!(definition.state.consumer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        destination(879, "EmitterCardAllocateChange", &args),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(879, "EmitterCardAllocateChange", &args));
    assert_eq!(runtime_event(879, "EmitterCardAllocateChange", 105), None);
    assert!(!subscribes_to_event(
        879,
        "EmitterCardAllocateChange",
        105,
        EventKind::RoundStartCard
    ));
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
        super::super::super::wire::find(510, "DamageNotMoreThan")
            .unwrap()
            .markers(super::super::super::wire::WirePhase::Add),
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
    assert!(has_destination(1008, "BanLostLife", &[160]));
    assert!(!has_destination(1008, "BanLostLife", &[500]));
    assert!(find(1008, "DamageNotMoreThan").is_none());
}

#[test]
fn hp_lock_is_an_exact_static_consumer() {
    let definition = find(796, "LockHp").unwrap();

    assert_eq!(definition.kind, BuffActKind::LockHp);
    assert!(definition.state.consumer);
    assert!(has_destination(796, "LockHp", &[10]));
    assert!(!has_destination(796, "LockHp", &[0]));
    assert!(find(796, "BanLostLife").is_none());

    let wire = super::super::super::wire::find(796, "LockHp").unwrap();
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Lockhp as i32]
    );
}

#[test]
fn missing_hp_healing_is_an_exact_static_consumer() {
    let definition = find(1011, "CureUpByLostHp").unwrap();

    assert_eq!(definition.kind, BuffActKind::CureUpByLostHp);
    assert!(definition.state.consumer);
    for args in [[200, 75, 8, 100], [100, 50, 8, 100], [100, 40, 8, 100]] {
        assert!(has_destination(1011, "CureUpByLostHp", &args));
    }
    assert!(!has_destination(1011, "CureUpByLostHp", &[100, 50, 8, 0]));
    assert!(!has_destination(1011, "CureUpByLostHp", &[100, 50, 8]));
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

    let wire = super::super::super::wire::find(793, "BurnRealHurtFix").unwrap();
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Realhurtfixwithlimit as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn real_hurt_fix_uses_only_its_captured_add_and_refresh_markers() {
    let wire = super::super::super::wire::find(519, "RealHurtFix").unwrap();
    let marker = sonettobuf::effect_type_enum::EffectType::Realhurtfix as i32;

    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[marker]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Refresh),
        &[marker]
    );
}

#[test]
fn dot_uses_only_its_captured_add_and_refresh_markers() {
    let wire = super::super::super::wire::find(202, "Dot").unwrap();
    let marker = sonettobuf::effect_type_enum::EffectType::Dot as i32;

    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Add),
        &[marker]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Refresh),
        &[marker]
    );
}

#[test]
fn add_to_attacker_marks_only_the_runtime_trigger() {
    let definition = find(305, "AddToAttacker").unwrap();
    let wire = super::super::super::wire::find(305, "AddToAttacker").unwrap();
    let marker = sonettobuf::effect_type_enum::EffectType::Addtoattacker as i32;

    assert_eq!(definition.kind, BuffActKind::AddToAttacker);
    assert_eq!(
        runtime_event(305, "AddToAttacker", 2091),
        Some(EventKind::BeAttacked)
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Add)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Static),
        &[marker]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
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
                .markers(super::super::super::wire::WirePhase::Add),
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
fn buff_type_duration_extension_is_an_exact_state_consumer() {
    let definition = find(608, "BuffRoundAddByBuffTypeId").unwrap();

    assert_eq!(definition.kind, BuffActKind::BuffRoundAddByBuffTypeId);
    assert_eq!(
        destination(608, "BuffRoundAddByBuffTypeId", &[6003, 1]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(608, "BuffRoundAddByBuffTypeId", &[]));
    assert!(!has_destination(
        608,
        "BuffRoundAddByBuffTypeId",
        &[6003, 0]
    ));
    assert!(find(608, "BuffRoundAdd").is_none());
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
fn rapport_scaled_critical_defense_is_an_exact_add_only_consumer() {
    use super::super::super::wire::WirePhase;
    use sonettobuf::effect_type_enum::EffectType;

    let args = [204, -150, 31460003, -30, 5, 204, 0];
    let definition = find(1141, "AddAttrByOtherBuffLayer").unwrap();

    assert_eq!(definition.kind, BuffActKind::AddAttrBySourceBuffLayer);
    assert!(!definition.runtime.effect_time_subscription);
    assert_eq!(
        definition.destination(),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(has_destination(1141, "AddAttrByOtherBuffLayer", &args));
    assert!(!has_destination(
        1141,
        "AddAttrByOtherBuffLayer",
        &args[..6]
    ));
    let wire = definition.wire.unwrap();
    assert_eq!(wire.markers(WirePhase::Add), &[EffectType::Attr as i32]);
    assert!(wire.markers(WirePhase::Static).is_empty());
    assert!(wire.markers(WirePhase::Refresh).is_empty());
    assert!(find(1029, "AddAttrByOtherBuffLayer").is_some());
    assert!(find(1036, "AddAttrByOtherBuffLayer").is_some());
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
    let wire = definition.wire.expect("injury has wire markers");
    for phase in [
        super::super::super::wire::WirePhase::Add,
        super::super::super::wire::WirePhase::Refresh,
    ] {
        assert_eq!(
            wire.markers(phase),
            &[sonettobuf::effect_type_enum::EffectType::Injury as i32]
        );
    }
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
