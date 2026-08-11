use super::*;

#[test]
fn add_enriches_buff_from_config_and_allocates_transaction_uid() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(41),
                    buff_id: Some(101),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager.add(&HpManager::default(), 10, 10, 101, 2).unwrap();

    assert_eq!(added.buff.uid, Some(43));
    assert_eq!(added.buff.buff_id, Some(101));
    assert_eq!(added.buff.from_uid, Some(10));
    assert_eq!(added.buff.layer, Some(2));
    assert_eq!(added.buff.count, Some(0));
    assert_eq!(manager.buff_type_amount(10, 1000), 3);
}

#[test]
fn version_seven_shares_one_buff_uid_lane_across_both_sides() {
    init_config();
    let fight = |version| Fight {
        version: Some(version),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let hp = HpManager::default();

    let mut current = BuffManager::default();
    current.seed(&fight(7));
    let attacker = current.add(&hp, 10, 10, 101, 1).unwrap();
    let defender = current.add(&hp, -1, -1, 101, 1).unwrap();
    assert_eq!(attacker.buff.uid, Some(1002));
    assert_eq!(defender.buff.uid, Some(1004));

    let mut legacy = BuffManager::default();
    legacy.seed(&fight(6));
    let attacker = legacy.add(&hp, 10, 10, 101, 1).unwrap();
    let defender = legacy.add(&hp, -1, -1, 101, 1).unwrap();
    assert_eq!(attacker.buff.uid, Some(2));
    assert_eq!(defender.buff.uid, Some(100001));
}

#[test]
fn visible_layer_carrier_grants_keep_shared_uids_consecutive() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10_i64, 11, 12, 13]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let hp = HpManager::default();
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let carrier_uids = [10_i64, 11, 12, 13]
        .into_iter()
        .map(|uid| manager.add(&hp, uid, uid, 31430141, 1).unwrap().buff.uid)
        .collect::<Vec<_>>();
    let following = manager.add(&hp, 10, 10, 101, 0).unwrap();

    assert_eq!(
        carrier_uids,
        [Some(1002), Some(1003), Some(1004), Some(1005)]
    );
    assert_eq!(following.buff.uid, Some(1007));
}

#[test]
fn repeated_condition_grant_uses_the_final_layer_child_uid() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let repeated =
        manager.add_repeated_replacing_excluded(&HpManager::default(), 10, 10, 31170006, 4);
    let following = manager
        .add(&HpManager::default(), 10, 10, 31080141, 0)
        .unwrap();

    assert_eq!(repeated.added.unwrap().buff.uid, Some(4));
    assert_eq!(following.buff.uid, Some(8));
}

#[test]
fn hidden_attr_three_stack_grants_use_consecutive_child_uids() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10_i64, 11, 12, 13]
                .into_iter()
                .enumerate()
                .map(|(index, uid)| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    buffs: if index == 0 {
                        vec![BuffInfo {
                            uid: Some(1141),
                            buff_id: Some(101),
                            ..Default::default()
                        }]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let carrier_uids = [10_i64, 11, 12, 13]
        .into_iter()
        .map(|uid| {
            manager
                .add_replacing_excluded_with_layer_specified(
                    &HpManager::default(),
                    uid,
                    uid,
                    435421,
                    0,
                    false,
                )
                .added
                .unwrap()
                .buff
                .uid
        })
        .collect::<Vec<_>>();
    let following = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    assert_eq!(
        carrier_uids,
        [Some(1142), Some(1143), Some(1144), Some(1145)]
    );
    assert_eq!(following.buff.uid, Some(1147));
}

#[test]
fn timed_independent_stack_does_not_reserve_an_unobserved_uid_slot() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1044),
                    buff_id: Some(31280113),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let timed = manager
        .add(&HpManager::default(), 10, 10, 31280114, 1)
        .unwrap();
    let following = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    assert_eq!(timed.buff.uid, Some(1045));
    assert_eq!(following.buff.uid, Some(1047));
}

#[test]
fn defender_buffs_use_defender_uid_band() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-10),
                team_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let added = manager
        .add(&HpManager::default(), -10, -10, 101, 1)
        .unwrap();

    assert_eq!(added.buff.uid, Some(100001));
}
