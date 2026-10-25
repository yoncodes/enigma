use super::*;

#[test]
fn ordinary_grant_keeps_origin_and_uses_manager_uid_policy() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };

    let plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: -1,
                target_uid: -1,
                buff_id: 70015,
                amount: Some(0),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let grant = grant_plan(&plan);
    assert_eq!(grant.uid.unwrap().uid, 100002);
    let changes = manager.commit(&HpManager::default(), plan);

    assert_eq!(changes.origin, origin);
    assert_eq!(
        changes.change.added.as_ref().unwrap().buff.uid,
        Some(100002)
    );
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::BuffAdded(event)]
            if event.source_uid == -1
                && event.target_uid == -1
                && event.buff_uid == 100002
                && event.buff_id == 70015
    ));
    assert_eq!(manager.added_history_for_owner(-1), &[70015]);
}
