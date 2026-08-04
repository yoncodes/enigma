use super::*;

#[test]
fn burn_layer_limit_uses_only_the_target_owners_modifier() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30940162),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    ..Default::default()
                },
            ],
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
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let hp = HpManager::default();

    manager.add_replacing_excluded(&hp, 10, 10, 4150001, 100);
    manager.add_replacing_excluded(&hp, 10, 11, 4150001, 100);
    manager.add_replacing_excluded(&hp, 10, -1, 4150001, 100);

    assert_eq!(manager.buff_id_amount(10, 4150001), 45);
    assert_eq!(manager.buff_id_amount(11, 4150001), 30);
    assert_eq!(manager.buff_id_amount(-1, 4150001), 30);
    assert_eq!(manager.grant_overflow(10, 10, 4150001, 1), 1);
    assert_eq!(manager.grant_overflow(10, -1, 4150001, 1), 1);
}

#[test]
fn capped_stack_does_not_create_a_loose_copy() {
    init_config();
    let fight = Fight {
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
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    manager.add_replacing_excluded(&HpManager::default(), 10, 10, 90071, 20);
    manager.add_replacing_excluded(&HpManager::default(), 10, 10, 90071, 10);
    let capped = manager.add_replacing_excluded(&HpManager::default(), 10, 10, 90071, 2);

    assert!(capped.added.is_none());
    assert!(capped.refreshed.is_empty());
    assert_eq!(manager.active_for(10).count(), 1);
    assert_eq!(manager.buff_id_amount(10, 90071), 30);
    assert_eq!(manager.added_count_for_team(1, &[90071]), 30);
}

#[test]
fn capped_layer_echo_buff_keeps_its_no_change_refresh() {
    init_config();
    let mut manager = BuffManager::default();
    manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        434015,
        20,
        true,
    );

    let result = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        434015,
        1,
        true,
    );

    assert_eq!(result.refreshed.len(), 1);
    assert_eq!(result.refreshed[0].before.layer, Some(20));
    assert_eq!(result.refreshed[0].after.layer, Some(20));
}

#[test]
fn typed_count_buff_refreshes_existing_copy() {
    init_config();
    let fight = Fight {
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
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let first = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        30631,
        0,
        false,
    );
    let second = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        30631,
        0,
        false,
    );

    assert_eq!(first.added.unwrap().buff.uid, Some(2));
    assert!(second.added.is_none());
    assert_eq!(second.refreshed.len(), 1);
    assert_eq!(second.refreshed[0].after.uid, Some(2));
    assert_eq!(second.refreshed[0].after.layer, Some(0));
    assert_eq!(second.refreshed[0].after.count, Some(2));
    assert_eq!(manager.active_for(10).count(), 1);

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(4));
}

#[test]
fn typed_count_buff_repeat_adds_then_refreshes_same_copy() {
    init_config();
    let fight = Fight {
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
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let result = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        30631,
        2,
        true,
    );

    let added = result.added.unwrap();
    assert_eq!(added.buff.uid, Some(2));
    assert_eq!(added.buff.layer, Some(0));
    assert_eq!(added.buff.count, Some(1));
    assert_eq!(result.refreshed.len(), 1);
    assert_eq!(result.refreshed[0].after.uid, Some(2));
    assert_eq!(result.refreshed[0].after.layer, Some(0));
    assert_eq!(result.refreshed[0].after.count, Some(2));
    assert_eq!(manager.active_for(10).count(), 1);

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(5));
}

#[test]
fn reapply_include_type_replaces_effect_count_buff() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let hp = HpManager::default();

    let first = manager.add_replacing_excluded(&hp, -2, -2, 530000712, 0);
    let second = manager.add_replacing_excluded(&hp, -2, -2, 530000712, 0);

    assert_eq!(first.added.unwrap().buff.uid, Some(100001));
    assert_eq!(second.removed.len(), 1);
    assert_eq!(second.removed[0].buff.uid, Some(100001));
    assert_eq!(second.added.unwrap().buff.uid, Some(100004));
    assert!(second.refreshed.is_empty());
}

#[test]
fn duplicate_policy_distinguishes_timed_reapplication_from_single_replacement() {
    init_config();

    assert_eq!(
        BuffPolicy::for_buff_id(30560101)
            .expect("poison definition")
            .on_duplicate,
        DuplicateGrant::AddSeparateCopy
    );
    assert_eq!(
        BuffPolicy::for_buff_id(30560101)
            .expect("poison definition")
            .storage,
        BuffStorage::SeparateCopies
    );
    assert_eq!(
        BuffPolicy::for_buff_id(101)
            .expect("attribute definition")
            .on_duplicate,
        DuplicateGrant::ReplaceExisting
    );
    let advancement = BuffPolicy::for_buff_id(30860131).expect("Advancement definition");
    assert_eq!(advancement.storage, BuffStorage::Single);
    assert_eq!(advancement.on_duplicate, DuplicateGrant::ReplaceExisting);
    let timed_layered = BuffPolicy::for_buff_id(7111).expect("timed layered definition");
    assert_eq!(timed_layered.storage, BuffStorage::Layered);
    assert_eq!(
        timed_layered.match_existing,
        ExistingBuffMatch::SameIdAndDuration
    );
    assert_eq!(timed_layered.on_duplicate, DuplicateGrant::MergeExisting);
}

#[test]
fn include_type_four_stores_timed_attribute_grants_as_separate_copies() {
    init_config();
    let policy = BuffPolicy::for_buff_id(30600101).expect("timed attribute definition");

    assert_eq!(policy.storage, BuffStorage::SeparateCopies);
    assert_eq!(policy.on_duplicate, DuplicateGrant::AddSeparateCopy);

    let hp = HpManager::default();
    let mut manager = BuffManager::default();
    manager.add_replacing_excluded(&hp, 10, 10, 30600101, 0);
    manager.add_replacing_excluded(&hp, 10, 10, 30600101, 0);

    assert_eq!(manager.active_for(10).count(), 2);
}

#[test]
fn include_type_five_uses_one_permanent_mechanic_carrier() {
    init_config();
    let policy = BuffPolicy::for_buff_id(31050145).expect("force-field carrier definition");

    assert_eq!(policy.storage, BuffStorage::Single);
    assert_eq!(policy.match_existing, ExistingBuffMatch::SameId);
    assert_eq!(policy.on_duplicate, DuplicateGrant::ReplaceExisting);
    assert_eq!(policy.uid.allocation, UidAllocationPolicy::Normal);
    assert!(policy.unresolved_include_entries.is_empty());
}

#[test]
fn shared_type_family_replaces_the_resident_variant() {
    init_config();
    let hp = HpManager::default();
    let mut manager = BuffManager::default();

    let first = manager.add_replacing_excluded(&hp, 10, 10, 400401, 0);
    let first_uid = first.added.expect("rank-one family member").buff.uid;
    let second = manager.add_replacing_excluded(&hp, 10, 10, 400403, 0);
    let second_uid = second
        .added
        .as_ref()
        .expect("rank-two family member")
        .buff
        .uid;

    let policy = BuffPolicy::for_buff_id(400403).unwrap();
    assert!(policy.unresolved_include_entries.is_empty());
    assert_eq!(policy.storage, BuffStorage::Single);
    assert_eq!(policy.match_existing, ExistingBuffMatch::SharedTypeFamily);
    assert_eq!(policy.on_duplicate, DuplicateGrant::ReplaceExisting);
    assert_eq!(second.removed.len(), 1);
    assert_eq!(second.removed[0].buff.buff_id, Some(400401));
    assert_eq!(second.removed[0].buff.uid, first_uid);
    assert_ne!(second_uid, first_uid);
    assert_eq!(
        manager
            .active_for(10)
            .filter_map(|buff| buff.buff_id)
            .collect::<Vec<_>>(),
        vec![400403]
    );
}

#[test]
fn include_type_seventeen_stores_capped_separate_counter_instances() {
    init_config();
    let policy = BuffPolicy::for_buff_id(23390015).expect("Manifest Dream counter definition");

    assert_eq!(policy.storage, BuffStorage::SeparateCopies);
    assert_eq!(policy.on_duplicate, DuplicateGrant::AddSeparateCopy);
    assert_eq!(policy.instance_limit, Some(3));

    let hp = HpManager::default();
    let mut manager = BuffManager::default();
    for _ in 0..3 {
        let change = manager.add_replacing_excluded(&hp, -99999, -99999, 23390015, 0);
        assert!(change.added.is_some());
    }
    let capped = manager.add_replacing_excluded(&hp, -99999, -99999, 23390015, 0);

    assert_eq!(manager.active_for(-99999).count(), 3);
    assert_eq!(manager.buff_type_amount(-99999, 23390015), 3);
    assert!(capped.added.is_none());
    assert!(capped.refreshed.is_empty());
    assert!(capped.removed.is_empty());
    assert!(capped.rejected.is_none());
}

#[test]
fn value_bearing_type_seven_evicts_the_oldest_same_type_instance() {
    init_config();
    let policy = BuffPolicy::for_buff_id(6200501).expect("Incantation Might definition");

    assert_eq!(policy.storage, BuffStorage::SeparateCopies);
    assert_eq!(policy.on_duplicate, DuplicateGrant::AddSeparateCopy);
    assert_eq!(policy.same_type_capacity, Some(10));
    assert!(policy.unresolved_include_entries.is_empty());

    let hp = HpManager::default();
    let mut manager = BuffManager::default();
    let definition = BuffDefinition::get(6200501).expect("shared type definition");
    manager.buffs.push(ActiveBuff {
        owner_uid: 10,
        team_type: 1,
        type_id: definition.effective_type_id(),
        definition: Some(definition),
        buff: BuffInfo {
            buff_id: Some(6200599),
            uid: Some(1),
            ..Default::default()
        },
    });
    for _ in 0..9 {
        let change = manager.add_replacing_excluded(&hp, 10, 10, 6200501, 0);
        change.added.expect("same-type instance");
    }

    let overflow = manager.add_replacing_excluded(&hp, 10, 10, 6200501, 0);

    assert_eq!(manager.buff_type_amount(10, 6200501), 10);
    assert_eq!(overflow.removed.len(), 1);
    assert_eq!(overflow.removed[0].buff.buff_id, Some(6200599));
    assert_eq!(overflow.removed[0].buff.uid, Some(1));
    assert_eq!(
        overflow.removed[0].delete_reason,
        Some(BuffDeleteReason::Overflow)
    );
    assert!(overflow.added.is_some());
}

#[test]
fn timed_layer_grants_only_merge_with_an_instance_at_the_fresh_duration() {
    init_config();
    let hp = HpManager::default();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(31050111),
                    duration: Some(2),
                    uid: Some(40),
                    from_uid: Some(10),
                    layer: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let first = manager.add_replacing_excluded_with_layer_specified(&hp, 10, 10, 31050111, 1, true);

    assert!(first.removed.is_empty());
    assert!(first.refreshed.is_empty());
    let fresh = first.added.expect("fresh-duration Gust instance").buff;
    assert_eq!(fresh.duration, Some(3));
    assert_eq!(fresh.layer, Some(1));
    assert_eq!(manager.snapshot(10, 40).unwrap().layer, Some(10));

    let second =
        manager.add_replacing_excluded_with_layer_specified(&hp, 10, 10, 31050111, 1, true);

    assert!(second.added.is_none());
    assert_eq!(second.refreshed.len(), 1);
    assert_eq!(second.refreshed[0].before.uid, fresh.uid);
    assert_eq!(second.refreshed[0].after.uid, fresh.uid);
    assert_eq!(second.refreshed[0].after.duration, Some(3));
    assert_eq!(second.refreshed[0].after.layer, Some(2));
    assert_eq!(manager.active_for(10).count(), 2);
}
