use super::*;

fn death_refresh_manager() -> BuffManager {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1108),
                        buff_id: Some(103374115),
                        from_uid: Some(10),
                        layer: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1154),
                        buff_id: Some(101),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1109),
                        buff_id: Some(103374114),
                        from_uid: Some(-2),
                        layer: Some(2),
                        duration: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    manager
}

fn death_refresh_command() -> BuffCommand {
    BuffCommand::GrantOnDeath(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 103374115,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    })
}

fn capped_death_grant_command() -> BuffCommand {
    BuffCommand::GrantOnDeath(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: -3,
        target_uid: 10,
        buff_id: 103374114,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    })
}

#[test]
fn capped_death_grant_owns_the_group_child_uid() {
    let mut manager = death_refresh_manager();

    let refresh = manager
        .plan(&HpManager::default(), death_refresh_command())
        .unwrap();
    let refresh_grant = grant_plan(&refresh);
    assert!(refresh_grant.pre_add_uids.is_empty());
    assert_eq!(refresh_grant.layer_refresh_uid.unwrap().uid, 1155);
    manager.commit(&HpManager::default(), refresh);

    let capped = manager
        .plan(&HpManager::default(), capped_death_grant_command())
        .unwrap();
    let capped_grant = grant_plan(&capped);
    assert_eq!(capped_grant.action, GrantAction::KeepExisting);

    assert_eq!(
        capped_grant
            .pre_add_uids
            .iter()
            .map(|plan| plan.uid)
            .collect::<Vec<_>>(),
        vec![1156]
    );
    manager.commit(&HpManager::default(), capped);
    let following = manager
        .add(&HpManager::default(), 10, 10, 70015, 0)
        .unwrap();
    assert_eq!(following.buff.uid, Some(1158));
}

#[test]
fn added_death_grant_uses_the_group_child_uid() {
    let mut manager = death_refresh_manager();
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60010, "DisperseForce2"),
                },
                target_uid: 10,
                selector: BuffRemoveSelector::ExactId(103374114),
            }),
        )
        .unwrap();

    let refresh = manager
        .plan(&HpManager::default(), death_refresh_command())
        .unwrap();
    assert_eq!(grant_plan(&refresh).layer_refresh_uid.unwrap().uid, 1155);
    manager.commit(&HpManager::default(), refresh);

    let added = manager
        .execute(&HpManager::default(), capped_death_grant_command())
        .unwrap();
    assert_eq!(added.change.added.unwrap().buff.uid, Some(1156));
    let following = manager
        .add(&HpManager::default(), 10, 10, 70015, 0)
        .unwrap();
    assert_eq!(following.buff.uid, Some(1158));
}
