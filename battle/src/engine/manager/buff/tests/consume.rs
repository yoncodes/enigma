use super::*;

#[test]
fn consuming_clarified_topic_type_decrements_count_not_layer() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(30631),
                    uid: Some(2),
                    count: Some(3),
                    layer: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let change = manager.consume_by_type_or_id(10, 8178, 1).unwrap();

    assert_eq!(change.after.count, Some(2));
    assert_eq!(change.after.layer, Some(0));
}

#[test]
fn consuming_stack_buff_decrements_layer_not_trigger_count() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(530000111),
                    uid: Some(100002),
                    count: Some(1),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let change = manager.consume_by_type_or_id(-1, 530000111, 1).unwrap();

    assert_eq!(change.after.count, Some(1));
    assert_eq!(change.after.layer, Some(2));
    assert_eq!(manager.attribute_delta(-1, AttrId::DmgTakenReduction), 600);

    let change = manager.consume_by_type_or_id(-1, 530000111, 1).unwrap();

    assert_eq!(change.after.layer, Some(1));
    assert_eq!(manager.attribute_delta(-1, AttrId::DmgTakenReduction), 300);
}

#[test]
fn consuming_last_stack_removes_buff_without_zero_refresh() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    buff_id: Some(530000111),
                    uid: Some(100002),
                    count: Some(1),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let result = manager.consume_by_type_or_id_replacing(-1, 530000111, 1);

    assert_eq!(result.removed.len(), 1);
    assert!(result.refreshed.is_empty());
    assert!(!manager.has_buff_id(-1, 530000111));
    assert_eq!(manager.attribute_delta(-1, AttrId::DmgTakenReduction), 0);
}

#[test]
fn stacked_buff_refresh_respects_include_type_limit() {
    init_config();
    let mut manager = BuffManager::default();
    manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        530000111,
        3,
        true,
    );

    let result = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        530000111,
        1,
        true,
    );

    assert!(result.refreshed.is_empty());
}

#[test]
fn layer_update_consumes_attempt_uid_but_capped_refresh_does_not() {
    init_config();
    let hp = HpManager::default();
    let mut manager = BuffManager::default();

    let first = manager
        .add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434725, 1, true)
        .added
        .unwrap();
    assert_eq!(first.buff.uid, Some(2));

    let update = manager.add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434725, 1, true);
    assert_eq!(update.refreshed[0].after.uid, Some(2));

    let after_update = manager
        .add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434735, 1, true)
        .added
        .unwrap();
    assert_eq!(after_update.buff.uid, Some(4));

    let mut capped = BuffManager::default();
    capped.add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434725, 30, true);
    let no_change =
        capped.add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434725, 1, true);
    assert!(no_change.refreshed.is_empty());

    let after_cap = capped
        .add_replacing_excluded_with_layer_specified(&hp, 10, 10, 434735, 1, true)
        .added
        .unwrap();
    assert_eq!(after_cap.buff.uid, Some(3));
}

#[test]
fn featured_count_buff_emits_each_intermediate_count() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let result = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        31020111,
        5,
        true,
    );

    let added = result.added.unwrap();
    assert_eq!(added.buff.count, Some(1));
    assert_eq!(added.buff.layer, Some(0));
    assert_eq!(
        result
            .refreshed
            .iter()
            .map(|update| update.after.count.unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
}

#[test]
fn master_halo_fans_linked_buff_to_allies_with_child_uids() {
    init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(12),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);

    let added = manager.add(&hp, 10, 10, 30860153, 2).unwrap();

    assert_eq!(added.buff.uid, Some(2));
    assert_eq!(added.markers[0].effect_type, 172);
    assert_eq!(added.fanout.len(), 1);
    assert_eq!(added.fanout[0].buff.buff_id, Some(30860162));
    assert_eq!(added.fanout[0].buff.uid, Some(3));
    assert_eq!(added.fanout[0].markers[0].effect_type, 173);

    let removed = manager.remove_by_id(10, 30860153);

    assert_eq!(
        removed
            .iter()
            .map(|change| (change.target_uid, change.buff.buff_id))
            .collect::<Vec<_>>(),
        vec![(11, Some(30860162)), (10, Some(30860153))]
    );
    assert!(!manager.has_buff_id(10, 30860162));
    assert!(!manager.has_buff_id(11, 30860162));

    let team = fight.attacker.as_mut().unwrap();
    std::mem::swap(&mut team.entitys[1], &mut team.sub_entitys[0]);
    manager.sync_roster(&fight);
    let promoted = manager.add(&hp, 10, 10, 30860153, 2).unwrap();
    assert_eq!(
        promoted
            .fanout
            .iter()
            .map(|buff| buff.target_uid)
            .collect::<Vec<_>>(),
        vec![12]
    );
}

#[test]
fn a_dispel_skips_buffs_that_cannot_be_dispelled() {
    init_config();
    let buff = |uid, buff_id| BuffInfo {
        buff_id: Some(buff_id),
        uid: Some(uid),
        duration: Some(1),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![buff(2, 4011), buff(3, 109380007)],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);

    managers
        .execute_buff(BuffCommand::Dispel(BuffDispel {
            origin: CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(20020, "PurifyX"),
            },
            target_uid: 10,
            statuses: vec![
                BuffStatus::from_id(2),
                BuffStatus::from_id(4),
                BuffStatus::from_id(6),
            ],
            excluded_ids_or_types: Vec::new(),
            count: 0,
        }))
        .unwrap();

    // [Skyward Gaze]: "(undispellable, stackable up to 5 times)".
    assert!(!managers.buff.has_active_buff_id(10, 4011));
    assert!(managers.buff.has_active_buff_id(10, 109380007));
}
