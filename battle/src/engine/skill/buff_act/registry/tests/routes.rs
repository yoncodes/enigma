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
fn state_carrying_buffs_mark_their_add_only() {
    use super::super::super::wire::WirePhase;
    use sonettobuf::effect_type_enum::EffectType;

    for (opcode, type_name) in [
        (1139, "MeiLeiErCharge"),
        (1146, "FoundationShield"),
        (1147, "FoundationCounter"),
    ] {
        let wire = find(opcode, type_name).unwrap().wire.unwrap();
        assert_eq!(wire.markers(WirePhase::Add), &[EffectType::None as i32]);
        assert!(wire.markers(WirePhase::Static).is_empty());
        assert!(wire.markers(WirePhase::Refresh).is_empty());
    }
}

#[test]
fn reflect_frames_are_owned_by_holder_and_mark_attacker() {
    let rebound = find(303, "Rebound").unwrap();
    assert_eq!(rebound.runtime.frame_source, RuntimeFrameSource::Owner);
    assert!(rebound.runtime.marker.is_none());

    let damage_based = find(743, "ReboundBasedOnDamage").unwrap();
    assert_eq!(damage_based.runtime.frame_source, RuntimeFrameSource::Owner);
    let marker = damage_based.runtime.marker.unwrap();
    assert_eq!(marker.position, RuntimeMarkerPosition::BeforeChanges);
    assert_eq!(marker.target, RuntimeMarkerTarget::EventSource);
    assert_eq!(damage_based.kind, BuffActKind::ReboundBasedOnDamage);
    assert!(damage_based.supports.unwrap()(&[300, 0, 0]));
    assert!(!damage_based.supports.unwrap()(&[300, 101, 2_000]));
    assert!(!damage_based.supports.unwrap()(&[150, 102, 1_000]));
    let wire = damage_based.wire.unwrap();
    assert!(!wire.has_output());
    assert_eq!(
        marker.effect_type,
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
    assert!(has_destination(202, "Dot", &[0, 101, 50]));
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
fn shield_counter_requires_the_configured_riposte_timing() {
    let game = crate::test_support::game_data();
    let mut feature = resolve_feature(Some(game), "802#30940171").unwrap();
    assert!(feature.arguments_supported);
    assert_eq!(
        destination_for_feature(Some(game), &feature),
        Some(BuffActDestination::StateConsumer)
    );
    feature.effect_time = 0;
    assert_eq!(destination_for_feature(Some(game), &feature), None);
}
