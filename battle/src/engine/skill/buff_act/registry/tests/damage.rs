use super::*;

#[test]
fn share_hurt_marks_adds_and_refreshes() {
    let wire = find(872, "ShareHurt").unwrap().wire.unwrap();

    for phase in [
        super::super::super::wire::WirePhase::Add,
        super::super::super::wire::WirePhase::Refresh,
    ] {
        assert_eq!(
            wire.markers(phase),
            &[sonettobuf::effect_type_enum::EffectType::None as i32]
        );
    }
}

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
fn foundation_shield_is_an_exact_snapshotted_damage_consumer() {
    let definition = find(1146, "FoundationShield").unwrap();

    assert_eq!(definition.kind, BuffActKind::FoundationShield);
    assert_eq!(
        definition.state.initial,
        Some(BuffActInitialStateRule::SourceAttack)
    );
    assert!(
        !definition
            .wire
            .expect("FoundationShield has static wire metadata")
            .initial_state_marker
    );
    assert_eq!(
        destination(1146, "FoundationShield", &[102, 800, 31_400_007, 1600]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(
        1146,
        "FoundationShield",
        &[101, 800, 31_400_007, 1600]
    ));
    assert!(find(1146, "FoundationCounter").is_none());
}

#[test]
fn foundation_counter_is_an_exact_snapshotted_state_consumer() {
    let definition = find(1147, "FoundationCounter").unwrap();

    assert_eq!(definition.kind, BuffActKind::FoundationCounter);
    assert_eq!(
        definition.state.initial,
        Some(BuffActInitialStateRule::FoundationCounter)
    );
    assert!(definition.state.consumer);
    assert!(definition.accepts_route(203, 0));
    assert!(!definition.accepts_route(202, 0));
    assert!(!definition.accepts_route(203, 1));
    assert_eq!(
        destination(1147, "FoundationCounter", &[5000, 1000, 1000]),
        Some(BuffActDestination::StateConsumer)
    );
    assert!(!has_destination(
        1147,
        "FoundationCounter",
        &[0, 1000, 1000]
    ));
    assert!(find(1147, "FoundationShield").is_none());
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
