use std::collections::HashMap;

use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    manager::{BattleManagers, buff::ActiveBuffFeature},
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::{ShadowCloak, capacity_rule_groups};

#[test]
fn initializes_only_when_raspberry_feature_exists() {
    let mut shadow = ShadowCloak::default();

    assert_eq!(shadow.init_from_features(&[]), None);
    assert!(!shadow.enabled());

    assert_eq!(
        shadow.init_from_features(&[feature(vec![1042, 100])]),
        Some(())
    );
    assert!(shadow.enabled());
}

#[test]
fn capacity_rules_keep_each_owners_operations_together() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10_i64, 11]
                .into_iter()
                .enumerate()
                .map(|(index, owner_uid)| FightEntityInfo {
                    uid: Some(owner_uid),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20 + index as i64),
                        buff_id: Some(31250151),
                        from_uid: Some(10),
                        act_common_params: Some("0#1500".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let groups = capacity_rule_groups(
        &managers,
        &[raspberry_feature(11, 21), raspberry_feature(10, 20)],
        &HashMap::from([((10, 20), 100)]),
    )
    .unwrap();

    assert_eq!(
        groups
            .iter()
            .map(|group| group.owner_uid)
            .collect::<Vec<_>>(),
        vec![11, 10]
    );
    assert!(groups.iter().all(|group| {
        matches!(
            group.operations.as_slice(),
            [RuleOp::Command(BattleCommand::RaspberryCapacity(command))]
                if command.target_uid == group.owner_uid
        )
    }));
}

#[test]
fn capacity_rules_do_not_share_a_source_between_teams() {
    crate::test_support::init_config();
    let entity = |uid, buff_uid, source_uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31250151),
            from_uid: Some(source_uid),
            act_common_params: Some("0#1500".to_owned()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 20, 10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(30, 40, 30)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let groups = capacity_rule_groups(
        &managers,
        &[
            raspberry_feature_on(10, 20, 10, 1),
            raspberry_feature_on(30, 40, 30, 2),
        ],
        &HashMap::from([((10, 20), 100)]),
    )
    .unwrap();

    assert_eq!(
        groups
            .iter()
            .map(|group| group.owner_uid)
            .collect::<Vec<_>>(),
        vec![10]
    );
}

#[test]
fn capacity_rules_reject_conflicting_team_sources() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();

    assert_eq!(
        capacity_rule_groups(
            &managers,
            &[
                raspberry_feature_on(10, 20, 10, 1),
                raspberry_feature_on(11, 21, 11, 1),
            ],
            &HashMap::from([((10, 20), 100)]),
        ),
        Err(super::CapacityPlanError::ConflictingSources {
            team_type: 1,
            sources: vec![10, 11],
        })
    );
}

#[test]
fn capacity_rules_ignore_unreached_team_source_conflicts() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31250151),
                    from_uid: Some(10),
                    act_common_params: Some("0#1500".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    let groups = capacity_rule_groups(
        &managers,
        &[
            raspberry_feature_on(10, 20, 10, 1),
            raspberry_feature_on(30, 40, 30, 2),
            raspberry_feature_on(31, 41, 31, 2),
        ],
        &HashMap::from([((10, 20), 100)]),
    )
    .unwrap();

    assert_eq!(
        groups
            .iter()
            .map(|group| group.owner_uid)
            .collect::<Vec<_>>(),
        vec![10]
    );
}

fn feature(values: Vec<i32>) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 1,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "Raspberry".to_owned(),
        effect_time: 103,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    }
}

fn raspberry_feature(owner_uid: i64, buff_uid: i64) -> ActiveBuffFeature {
    raspberry_feature_on(owner_uid, buff_uid, 10, 1)
}

fn raspberry_feature_on(
    owner_uid: i64,
    buff_uid: i64,
    source_uid: i64,
    team_type: i32,
) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid,
        buff_uid,
        buff_id: 31250151,
        amount: 1,
        team_type,
        owner_alive: true,
        act_type: "Raspberry".to_owned(),
        effect_time: 103,
        effect_condition: 0,
        raw: "1042#100#100#700#150#203#50#211#33#1#40".to_owned(),
        values: vec![1042, 100, 100, 700, 150, 203, 50, 211, 33, 1, 40],
    }
}
