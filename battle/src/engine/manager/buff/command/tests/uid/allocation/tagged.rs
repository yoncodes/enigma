use super::*;

fn tagged_group_fight(version: i32) -> Fight {
    Fight {
        version: Some(version),
        attacker: Some(FightTeam {
            entitys: (10..14)
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
    }
}

fn grant_tagged_group(manager: &mut BuffManager, target_uids: &[i64]) {
    let hp = HpManager::default();
    for target_uid in target_uids {
        manager
            .execute(
                &hp,
                BuffCommand::Grant(BuffGrant {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(1, "AddBuff"),
                    },
                    source_uid: 10,
                    target_uid: *target_uid,
                    buff_id: 31450132,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                }),
            )
            .unwrap();
    }
}

fn tagged_group_completion(manager: &BuffManager) -> BuffPlan {
    manager
        .plan(
            &HpManager::default(),
            BuffCommand::CompleteTaggedGroup(BuffTaggedGroupCompletion {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uids: vec![10, 11, 12, 13],
                buff_id: 31450132,
            }),
        )
        .unwrap()
}

#[test]
fn tagged_group_completion_reserves_only_after_every_member_received_the_buff() {
    crate::test_support::init_config();
    let mut complete = BuffManager::default();
    complete.seed(&tagged_group_fight(7));
    grant_tagged_group(&mut complete, &[10, 11, 12, 13]);
    assert_eq!(
        tagged_group_completion(&complete)
            .planned_reservation_uids()
            .unwrap()
            .len(),
        1
    );

    let mut incomplete = BuffManager::default();
    incomplete.seed(&tagged_group_fight(7));
    grant_tagged_group(&mut incomplete, &[10, 11, 12]);
    assert_eq!(
        tagged_group_completion(&incomplete).planned_reservation_uids(),
        Some(Vec::new())
    );
}

#[test]
fn tagged_group_completion_does_not_change_split_uid_lanes() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&tagged_group_fight(6));
    grant_tagged_group(&mut manager, &[10, 11, 12, 13]);

    assert_eq!(
        tagged_group_completion(&manager).planned_reservation_uids(),
        Some(Vec::new())
    );
}

#[test]
fn tagged_group_completion_reserves_only_on_its_first_application() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&tagged_group_fight(7));
    grant_tagged_group(&mut manager, &[10, 11, 12, 13]);
    assert_eq!(
        tagged_group_completion(&manager)
            .planned_reservation_uids()
            .unwrap()
            .len(),
        1
    );

    for target_uid in 10..14 {
        manager
            .execute(
                &HpManager::default(),
                BuffCommand::Remove(BuffRemove {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(60010, "DisperseForce2"),
                    },
                    target_uid,
                    selector: BuffRemoveSelector::ExactId(31450132),
                }),
            )
            .unwrap();
    }
    grant_tagged_group(&mut manager, &[10, 11, 12, 13]);

    assert_eq!(
        tagged_group_completion(&manager).planned_reservation_uids(),
        Some(Vec::new())
    );
}
