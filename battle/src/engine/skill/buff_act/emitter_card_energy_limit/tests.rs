use super::*;

fn feature(owner_uid: i64, delta: i32) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: 3_141,
        buff_uid: 1,
        buff_id: 31_410_401,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "ChangeEmitterCardEnergyLimit".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: format!("1078#{delta}"),
        values: vec![1_078, delta],
    }
}

#[test]
fn exact_static_owner_increases_only_the_emitter_card_limit() {
    let emitter_uid = crate::engine::manager::emitter::UID;

    assert!(supports(&[3]));
    assert!(!supports(&[0]));
    assert!(!supports(&[-1]));
    assert!(!supports(&[3, 1]));
    assert_eq!(
        super::super::registry::destination(1_078, "ChangeEmitterCardEnergyLimit", &[3]),
        Some(super::super::registry::BuffActDestination::StateConsumer)
    );
    let wire = super::super::registry::find(1_078, "ChangeEmitterCardEnergyLimit")
        .unwrap()
        .wire
        .unwrap();
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
    assert_eq!(resolve(5, &[feature(emitter_uid, 3)], emitter_uid), 8);
    assert_eq!(resolve(5, &[feature(3_141, 3)], emitter_uid), 5);
}
