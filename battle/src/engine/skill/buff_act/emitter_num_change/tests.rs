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
