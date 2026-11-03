use super::*;

fn feature(owner_uid: i64, skill_id: i32) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: 3141,
        buff_uid: 1,
        buff_id: 31411402,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "ChangeEmitterSkill".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: format!("1079#{skill_id}"),
        values: vec![1079, skill_id],
    }
}

#[test]
fn exact_static_owner_replaces_only_the_emitter_skill() {
    crate::test_support::init_config();
    let emitter_uid = crate::engine::manager::emitter::UID;
    let base = ImpromptuDefinition::new(2240001, 879, 150);
    let parsed =
        super::super::registry::resolve_feature(Some(config::configs::get()), "1079#31411001")
            .unwrap();

    assert!(supports(&[31410001]));
    assert!(supports(&[31411001]));
    assert!(!supports(&[0]));
    assert!(!supports(&[31411001, 1]));
    assert_eq!(
        super::super::registry::destination(1079, "ChangeEmitterSkill", &[31411001]),
        Some(super::super::registry::BuffActDestination::StateConsumer)
    );
    let wire = super::super::registry::find(1079, "ChangeEmitterSkill")
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
    assert_eq!(
        parsed.references(Some(config::configs::get())).skills,
        [31411001]
    );
    assert_eq!(
        resolve(Some(base), &[feature(emitter_uid, 31411001)], emitter_uid)
            .unwrap()
            .skill_id(),
        31411001
    );
    assert_eq!(
        resolve(Some(base), &[feature(3141, 31411001)], emitter_uid)
            .unwrap()
            .skill_id(),
        2240001
    );
}
