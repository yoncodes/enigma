use super::*;

#[test]
fn field_replacement_record_keeps_its_round_end_runtime_and_add_only_wire_contract() {
    use super::super::super::wire::WirePhase;
    use sonettobuf::effect_type_enum::EffectType;

    let definition = find(1084, "RecordReplaceWqMagicCircle").unwrap();

    assert!(definition.validate_arguments(None, &[], None));
    assert!(!definition.validate_arguments(None, &[1], None));
    assert_eq!(definition.destination(), Some(BuffActDestination::Runtime));
    assert_eq!(
        definition.runtime.event_override,
        Some(EventKind::RoundEndAfterFinalSettlement)
    );
    let wire = definition.wire.unwrap();
    assert_eq!(wire.markers(WirePhase::Add), &[EffectType::None as i32]);
    assert!(wire.markers(WirePhase::Static).is_empty());
    assert!(wire.markers(WirePhase::Refresh).is_empty());
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
        wire.markers(super::super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
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
        reaction_timing(1042, "Raspberry"),
        crate::engine::event::subscription::ReactionTiming::Immediate
    );
    assert_eq!(
        reaction_timing(1023, "LostHpAddExtraBloodPoolValue"),
        crate::engine::event::subscription::ReactionTiming::AfterSettlement
    );
}

#[test]
fn crystal_attack_grants_run_after_lethal_hit_settlement() {
    assert_eq!(
        reaction_timing(1051, "CrystalAddBuff"),
        crate::engine::event::subscription::ReactionTiming::AfterSettlement
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
fn layered_holder_dot_keeps_its_exact_round_start_route() {
    assert_eq!(runtime_event(213, "Dot", 101), Some(EventKind::RoundStart));
    assert!(has_destination(213, "Dot", &[1, 100, 30]));

    let wire = super::super::super::wire::find(213, "Dot").unwrap();
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Add)
            .is_empty()
    );
    assert_eq!(
        wire.markers(super::super::super::wire::WirePhase::Static),
        &[sonettobuf::effect_type_enum::EffectType::Dot as i32]
    );
    assert!(
        wire.markers(super::super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn moxie_loss_keeps_its_exact_routes() {
    assert_eq!(
        runtime_event(605, "ExPointDel", 302),
        Some(EventKind::RoundEnd)
    );
    assert!(has_destination(605, "ExPointDel", &[1]));
    assert!(!has_destination(605, "ExPointDel", &[0]));

    assert_eq!(
        runtime_event(609, "ExPointDel", 2081),
        Some(EventKind::SkillCast)
    );
    assert!(has_destination(609, "ExPointDel", &[1]));
    assert!(!has_destination(609, "ExPointDel", &[0]));
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
fn skill_target_count_attributes_keep_separate_exact_identities() {
    let single = find(106, "AttrSkillSingle").unwrap();
    let multiple = find(107, "AttrSkillMultiple").unwrap();

    assert_eq!(single.kind, BuffActKind::AttrSkillSingle);
    assert_eq!(multiple.kind, BuffActKind::AttrSkillMultiple);
    assert!(single.state.consumer);
    assert!(multiple.state.consumer);
    assert!(has_destination(106, "AttrSkillSingle", &[205, -300]));
    assert!(has_destination(107, "AttrSkillMultiple", &[205, -300]));
    assert!(!has_destination(106, "AttrSkillSingle", &[205]));
    assert!(!has_destination(107, "AttrSkillMultiple", &[205, 0]));
    assert!(!has_destination(106, "AttrSkillSingle", &[205, 300]));
    assert!(!has_destination(107, "AttrSkillMultiple", &[205, -200]));
    assert!(find(106, "AttrSkillMultiple").is_none());
    assert!(find(107, "AttrSkillSingle").is_none());
}

#[test]
fn damage_type_be_attacked_attribute_keeps_its_exact_identity() {
    assert!(has_destination(
        112,
        "AttrOnlyCalDamageBeAttacked",
        &[206, 250, 1]
    ));
    assert!(!has_destination(
        112,
        "AttrOnlyCalDamageBeAttacked",
        &[205, 250, 1]
    ));
    assert!(!has_destination(
        112,
        "AttrOnlyCalDamageBeAttacked",
        &[206, 250, 2]
    ));
    assert!(!has_destination(
        112,
        "AttrOnlyCalDamageBeAttacked",
        &[206, 250, 1, 1]
    ));
    let definition = find(114, "AttrOnlyCalDamageBeAttackedType").unwrap();

    assert_eq!(
        definition.kind,
        BuffActKind::AttrOnlyCalDamageBeAttackedType
    );
    assert!(definition.state.consumer);
    assert!(has_destination(
        114,
        "AttrOnlyCalDamageBeAttackedType",
        &[1, 206, 300, 1]
    ));
    assert!(has_destination(
        114,
        "AttrOnlyCalDamageBeAttackedType",
        &[2, 206, -250, 1]
    ));
    assert!(!has_destination(
        114,
        "AttrOnlyCalDamageBeAttackedType",
        &[0, 206, -250, 1]
    ));
    assert!(!has_destination(
        114,
        "AttrOnlyCalDamageBeAttackedType",
        &[2, 205, -250, 1]
    ));
    assert!(!has_destination(
        114,
        "AttrOnlyCalDamageBeAttackedType",
        &[2, 206, -250, 0]
    ));
    assert!(find(114, "AttrOnlyCalDamageBeAttacked").is_none());
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
fn special_moxie_cap_keeps_its_exact_transaction_identity() {
    let definition = find(832, "SpExPointMaxAdd").unwrap();

    assert_eq!(definition.kind, BuffActKind::SpExPointMaxAdd);
    assert_eq!(
        definition.transaction.events,
        &[
            EventKind::BuffAdded,
            EventKind::BuffChanged,
            EventKind::BuffRemoved,
        ]
    );
    assert!(has_destination(832, "SpExPointMaxAdd", &[7]));
    assert!(has_destination(832, "SpExPointMaxAdd", &[7, 3]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[0, 3]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[-7]));
    assert!(!has_destination(832, "SpExPointMaxAdd", &[7, -3]));
    assert!(find(832, "ExPointMaxAdd").is_none());
}

#[test]
fn transferred_moxie_keeps_its_exact_state_route() {
    let definition = find(833, "TransferAddExPoint").unwrap();

    assert_eq!(definition.kind, BuffActKind::TransferAddExPoint);
    assert_eq!(
        destination(833, "TransferAddExPoint", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(833, "TransferAddExPoint", &[1]));
    assert!(find(833, "ExPointCantAdd").is_none());
}

#[test]
fn counted_channel_keeps_its_exact_state_event_route() {
    let definition = find(838, "CountContinueChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::CountContinueChannel);
    assert_eq!(
        runtime_event(838, "CountContinueChannel", 1041),
        Some(EventKind::BuffStateChanged)
    );
    assert!(has_destination(
        838,
        "CountContinueChannel",
        &[31000505, 1, 210, 7]
    ));
    assert!(!has_destination(
        838,
        "CountContinueChannel",
        &[31000505, 0, 210, 7]
    ));
    let wire = super::super::super::wire::find(838, "CountContinueChannel").unwrap();
    assert_eq!(
        wire.initial_private_state(&[838, 31000505, 1, 210, 7]),
        Some(7)
    );
    assert_eq!(wire.initial_private_state(&[838, 31000505, 1, 210]), None);
    assert!(find(838, "CastChannel").is_none());
}

#[test]
fn contract_channel_keeps_its_exact_grant_and_round_start_routes() {
    let definition = find(836, "ContractCastChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::ContractCastChannel);
    assert_eq!(definition.transaction.events, &[EventKind::BuffAdded]);
    assert_eq!(
        runtime_publication(836, "ContractCastChannel", EventKind::BuffAdded),
        PublicationPhase::BeforePublish
    );
    assert_eq!(
        runtime_event(836, "ContractCastChannel", 104),
        Some(EventKind::RoundStart)
    );
    assert!(has_destination(
        836,
        "ContractCastChannel",
        &[1, 150, 31_000_151, 31_000_441]
    ));
    assert!(!has_destination(
        836,
        "ContractCastChannel",
        &[0, 150, 31_000_151, 31_000_441]
    ));
    assert!(!has_destination(
        836,
        "ContractCastChannel",
        &[1, -150, 31_000_151, 31_000_441]
    ));
    assert!(find(836, "CastChannel").is_none());
}

#[test]
fn bound_channel_lock_keeps_its_exact_state_route() {
    let definition = find(837, "NoneCastChannel").unwrap();

    assert_eq!(definition.kind, BuffActKind::NoneCastChannel);
    assert_eq!(
        destination(837, "NoneCastChannel", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(837, "NoneCastChannel", &[1]));
    assert!(find(837, "ContractCastChannel").is_none());
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
        let wire = super::super::super::wire::find(act_id, "AttrByLostHp").unwrap();
        assert_eq!(
            wire.markers(super::super::super::wire::WirePhase::Add),
            &[sonettobuf::effect_type_enum::EffectType::None as i32]
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
        !super::super::super::wire::find(402, "Petrified")
            .unwrap()
            .has_output()
    );
    assert!(find(402, "Dizzy").is_none());

    let frozen = find(404, "Frozen").unwrap();
    assert_eq!(frozen.kind, BuffActKind::Frozen);
    assert!(frozen.state.consumer);
    assert_eq!(
        runtime_event(404, "Frozen", 0),
        Some(EventKind::TargetAttacked)
    );
    assert!(has_destination(404, "Frozen", &[]));
    assert!(!has_destination(404, "Frozen", &[1]));
    assert!(find(404, "Petrified").is_none());
    assert_eq!(
        super::super::super::wire::find(404, "Frozen")
            .unwrap()
            .markers(super::super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Frozen as i32]
    );
}

#[test]
fn shock_wave_channel_keeps_its_exact_round_end_rule() {
    let definition = find(1031, "ConsumeBuffAddBuffContinueChannel").unwrap();

    assert_eq!(
        definition.kind,
        BuffActKind::ConsumeBuffAddBuffContinueChannel
    );
    assert_eq!(
        runtime_event(1031, "ConsumeBuffAddBuffContinueChannel", 302),
        Some(EventKind::RoundEnd)
    );
    assert!(has_destination(
        1031,
        "ConsumeBuffAddBuffContinueChannel",
        &[31280151, 31280113, 0, 50]
    ));
    assert!(!has_destination(
        1031,
        "ConsumeBuffAddBuffContinueChannel",
        &[31280151, 31280113, 20, 20]
    ));
    assert!(find(1031, "ConsumeBuffContinueChannel").is_none());
}

#[test]
fn hero_temp_cards_keep_their_exact_round_start_route() {
    let definition = find(739, "CreateHeroTempCards").unwrap();

    assert_eq!(definition.kind, BuffActKind::CreateHeroTempCards);
    assert_eq!(
        runtime_event(739, "CreateHeroTempCards", 105),
        Some(EventKind::RoundStartCard)
    );
    assert!(has_destination(739, "CreateHeroTempCards", &[1, 2, 1]));
    assert!(!has_destination(739, "CreateHeroTempCards", &[0, 2, 1]));
    assert!(!has_destination(739, "CreateHeroTempCards", &[1, 2, 2]));
    assert!(find(739, "AddSpTempCard").is_none());
}

#[test]
fn buff_owned_charge_keeps_its_exact_state_route() {
    crate::test_support::init_config();
    let definition = find(1139, "MeiLeiErCharge").unwrap();

    assert_eq!(definition.kind, BuffActKind::BuffOwnedCharge);
    assert_eq!(definition.completion_gap, None);
    for (buff_id, raw) in [
        (115370004, "1139#100000#150000#30110131"),
        (31460141, "1139#100000#150000#31460181"),
        (31460142, "1139#100000#150000#31460182"),
        (31460143, "1139#100000#150000#31460183"),
    ] {
        let configured = config::configs::get().skill_buff.get(buff_id).unwrap();
        assert_eq!(configured.features, raw);
        let values = crate::engine::entity::skill::split_ids(&configured.features);
        assert_eq!(
            destination(1139, "MeiLeiErCharge", &values[1..]),
            Some(BuffActDestination::StateConsumer)
        );
    }
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[0, 150_000, 31460183]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 99_999, 31460183]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 150_000, 0]
    ));
    assert!(!has_destination(
        1139,
        "MeiLeiErCharge",
        &[100_000, 150_000, 31460183, 1]
    ));
    assert!(find(1139, "HeatScaleUseSkill").is_none());
}

#[test]
fn bendith_rules_keep_distinct_exact_routes_and_structured_support() {
    crate::test_support::init_config();
    let replacement = find(1138, "ReplaceEntitySkillGroup").unwrap();
    assert_eq!(replacement.kind, BuffActKind::ReplaceEntitySkillGroup);
    assert_eq!(
        replacement.transaction.events,
        &[EventKind::BuffAdded, EventKind::BuffRemoved]
    );
    assert_eq!(
        destination_with_raw(
            config::try_get(),
            1138,
            "ReplaceEntitySkillGroup",
            &[],
            Some("1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223")
        ),
        Some(BuffActDestination::Transaction)
    );
    assert_eq!(
        destination_with_raw(
            config::try_get(),
            1138,
            "ReplaceEntitySkillGroup",
            &[],
            Some("1138#1:31460211#2:31460221")
        ),
        None
    );

    let no_cost = find(1140, "SkillNoUseActPoint").unwrap();
    assert_eq!(no_cost.kind, BuffActKind::SkillNoUseActPoint);
    assert_eq!(
        destination(1140, "SkillNoUseActPoint", &[]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(destination(1140, "SkillNoUseActPoint", &[1]), None);
    assert!(find(1140, "BigSkillNoUseActPoint").is_none());
}

#[test]
fn generic_feature_arguments_never_compact_invalid_cells() {
    crate::test_support::init_config();

    let invalid = resolve_feature(config::try_get(), "879#1#bad#2").unwrap();
    assert!(invalid.values.is_empty());
    assert_eq!(
        invalid.parse_error,
        Some(BuffActParseError::InvalidInteger { cell: 2, item: 0 })
    );

    let empty = resolve_feature(config::try_get(), "879#1##2").unwrap();
    assert!(empty.values.is_empty());
    assert_eq!(
        empty.parse_error,
        Some(BuffActParseError::EmptyArgument { cell: 2, item: 0 })
    );

    let valid = resolve_feature(config::try_get(), "879#1#300#1,2").unwrap();
    assert_eq!(valid.values, [879, 1, 300, 1, 2]);
    assert!(valid.parse_error.is_none());
}

#[test]
fn structured_feature_grammars_are_exact_registry_metadata() {
    crate::test_support::init_config();
    let game = config::configs::get();
    let mut structured = 0;

    for buff in game.skill_buff.iter() {
        for raw in buff
            .features
            .split('|')
            .map(str::trim)
            .filter(|raw| !raw.is_empty())
        {
            let Some(feature) = resolve_feature(Some(game), raw) else {
                continue;
            };
            let Some(definition) = feature.definition else {
                continue;
            };
            let cells = raw.split('#').map(str::to_owned).collect::<Vec<_>>();
            if parse_integer_projection(&cells).is_err() {
                structured += 1;
                assert!(
                    definition.parser.is_some(),
                    "registered structured feature lacks an exact parser: {raw}"
                );
                assert!(
                    feature.parse_error.is_none(),
                    "exact structured parser rejected configured feature: {raw}"
                );
            }
        }
    }

    assert!(structured > 0);
}

#[test]
fn structured_features_keep_semantic_values_and_references() {
    crate::test_support::init_config();
    let game = config::configs::get();

    let replacement = resolve_feature(
        Some(game),
        "1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223",
    )
    .unwrap();
    assert!(replacement.arguments_supported);
    assert_eq!(replacement.references(Some(game)).skills.len(), 6);

    let assassination = resolve_feature(
        Some(game),
        "10004#10#1#312401451,312401452:312401453,312401454",
    )
    .unwrap();
    assert!(assassination.arguments_supported);
    assert_eq!(
        assassination.values,
        [10004, 10, 1, 312401451, 312401452, 312401453, 312401454]
    );
    assert!(assassination.references(Some(game)).skills.is_empty());
    assert!(assassination.references(Some(game)).buffs.is_empty());

    let malformed = resolve_feature(Some(game), "10004#10#1#312401451,312401452:bad").unwrap();
    assert!(malformed.values.is_empty());
    assert!(malformed.is_malformed());

    let raspberry =
        resolve_feature(Some(game), "1042#100#100#700#150#203#50#211#33#1#40#26.4").unwrap();
    assert!(raspberry.arguments_supported);
    assert_eq!(raspberry.values.last(), Some(&264));
}
