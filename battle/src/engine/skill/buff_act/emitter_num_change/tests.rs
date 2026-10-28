use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn exact_emitter_count_route_requires_one_positive_count() {
    use super::super::registry::{BuffActDestination, destination};

    assert_eq!(
        destination(878, "EmitterNumChange", &[1]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(destination(878, "EmitterNumChange", &[0]), None);
    assert_eq!(destination(878, "AttrByShield", &[1]), None);
}

#[test]
fn exact_split_route_requires_positive_cadence_and_count() {
    use super::super::registry::{BuffActDestination, destination};

    assert_eq!(
        destination(892, "AttackNumSplitEmitterNum", &[1, 1, 2]),
        Some(BuffActDestination::StateConsumer)
    );
    assert_eq!(
        destination(892, "AttackNumSplitEmitterNum", &[1, 0, 2]),
        None
    );
    assert_eq!(
        destination(892, "AttackNumSplitEmitterNum", &[1, 1, 0]),
        None
    );
    assert_eq!(destination(892, "AddSplitEmitterNum", &[1, 1, 2]), None);
}

#[test]
fn exact_emitter_damage_modifiers_keep_distinct_support() {
    use super::super::{
        registry::{BuffActDestination, destination},
        wire::{self, WirePhase},
    };
    use sonettobuf::effect_type_enum::EffectType;

    let limit = super::super::registry::find(1080, "EmitterAttackNumLimitFixDamage").unwrap();
    assert!(limit.validate_arguments(None, &[4, 2500, 4], None));
    assert!(!limit.validate_arguments(None, &[4, 2500], None));
    assert!(!limit.validate_arguments(None, &[0, 2500, 4], None));
    assert_eq!(
        destination(1080, "EmitterAttackNumLimitFixDamage", &[4, 2500, 4]),
        Some(BuffActDestination::StateConsumer)
    );
    let limit_wire = wire::find(1080, "EmitterAttackNumLimitFixDamage").unwrap();
    assert_eq!(
        limit_wire.markers(WirePhase::Add),
        &[EffectType::None as i32]
    );
    assert!(limit_wire.markers(WirePhase::Static).is_empty());
    assert!(limit_wire.markers(WirePhase::Refresh).is_empty());

    let reduction =
        super::super::registry::find(1081, "EmitterFixSubTargetsDamageReduceRate").unwrap();
    assert!(reduction.validate_arguments(None, &[700], None));
    assert!(!reduction.validate_arguments(None, &[], None));
    assert!(!reduction.validate_arguments(None, &[0], None));
    assert_eq!(
        destination(1081, "EmitterFixSubTargetsDamageReduceRate", &[700]),
        Some(BuffActDestination::StateConsumer)
    );
    let reduction_wire = wire::find(1081, "EmitterFixSubTargetsDamageReduceRate").unwrap();
    assert_eq!(
        reduction_wire.markers(WirePhase::Add),
        &[EffectType::None as i32]
    );
    assert!(reduction_wire.markers(WirePhase::Static).is_empty());
    assert!(reduction_wire.markers(WirePhase::Refresh).is_empty());
    assert_eq!(
        destination(1081, "EmitterFixSubTargetsDamageReduceRate", &[0]),
        None
    );
}

#[test]
fn emitter_limit_converts_excess_attacks_to_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(30530302),
                        from_uid: Some(-2),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31410101),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31080152),
                        from_uid: Some(emitter::UID),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31080153),
                        from_uid: Some(emitter::UID),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31080154),
                        from_uid: Some(emitter::UID),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31080155),
                        from_uid: Some(emitter::UID),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31412602),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = BuffManager::default();
    buffs.seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    assert_eq!(
        resolve(&buffs, &hp, emitter::UID),
        Resolution {
            count: 4,
            damage_rate: 10_000,
        }
    );
    let limit = buffs
        .active_features(&hp)
        .into_iter()
        .find(|feature| is_kind(feature, BuffActKind::EmitterAttackNumLimitFixDamage))
        .unwrap();
    assert_eq!(attack_limit(&limit, 10), Some((4, 10_000)));
}

fn split_feature() -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: emitter::UID,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 31410408,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "AttackNumSplitEmitterNum".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "892#1#1#2".to_owned(),
        values: vec![892, 1, 1, 2],
    }
}

#[test]
fn attack_num_split_feature_splits_every_attack_into_two_more_targets() {
    let feature = split_feature();

    assert_eq!(attack_count_delta(&feature), 0);
    assert_eq!(split_count_on_attack(&feature, 1), 2);
    assert_eq!(split_count_on_attack(&feature, 2), 2);
    assert_eq!(split_count_on_attack(&feature, 3), 2);
}

#[test]
fn added_split_targets_apply_only_when_the_cadence_splits() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(30480241),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(30480211),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = BuffManager::default();
    buffs.seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    assert_eq!(split_count(&buffs, &hp, 1), 2);
    assert_eq!(split_count(&buffs, &hp, 2), 0);
    assert_eq!(split_count(&buffs, &hp, 3), 2);
}

#[test]
fn split_targets_use_the_default_final_damage_penalty() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    buff_id: Some(30480241),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = BuffManager::default();
    buffs.seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    assert_eq!(split_final_damage_delta(&buffs, &hp, 1), -700);
    assert_eq!(split_final_damage_delta(&buffs, &hp, 0), 0);
}

#[test]
fn configured_split_damage_reduction_overrides_the_default() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    buff_id: Some(114340006),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = BuffManager::default();
    buffs.seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    assert_eq!(split_final_damage_delta(&buffs, &hp, 1), -500);
}
