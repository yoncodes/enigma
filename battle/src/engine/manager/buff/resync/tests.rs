use super::*;
use sonettobuf::{FightEntityInfo, FightTeam};

fn manager(target_buff: Option<BuffInfo>) -> BuffManager {
    manager_with_buffs(target_buff.into_iter().collect())
}

fn manager_with_buffs(target_buffs: Vec<BuffInfo>) -> BuffManager {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(99),
                        buff_id: Some(31430145),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    buffs: target_buffs,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    manager.accumulate_act_value(99, 1128, 7);
    manager
}

fn observed_snapshot_buff() -> BuffInfo {
    BuffInfo {
        uid: Some(1),
        buff_id: Some(31430131),
        from_uid: Some(10),
        ..Default::default()
    }
}

#[test]
fn observed_insert_initializes_grant_time_state() {
    let mut manager = manager(None);

    assert!(
        manager
            .resync_observed_present(20, observed_snapshot_buff())
            .unwrap()
    );

    assert_eq!(manager.grant_value(1, 1127), Some(140));
}

#[test]
fn observed_id_change_rebuilds_definition_and_grant_time_state() {
    let mut manager = manager(Some(BuffInfo {
        uid: Some(1),
        buff_id: Some(31050111),
        from_uid: Some(20),
        ..Default::default()
    }));

    assert!(
        manager
            .resync_observed_present(20, observed_snapshot_buff())
            .unwrap()
    );

    assert_eq!(manager.grant_value(1, 1127), Some(140));
    assert_eq!(manager.buff_id_amount(20, 31050111), 0);
    assert_eq!(manager.buff_id_amount(20, 31430131), 1);
}

#[test]
fn observed_single_copy_replaces_the_generated_uid() {
    let mut manager = manager(Some(BuffInfo {
        uid: Some(1),
        buff_id: Some(109320111),
        from_uid: Some(20),
        layer: Some(1),
        ..Default::default()
    }));

    assert!(
        manager
            .resync_observed_present(
                20,
                BuffInfo {
                    uid: Some(2),
                    buff_id: Some(109320111),
                    from_uid: Some(20),
                    layer: Some(2),
                    ..Default::default()
                }
            )
            .unwrap()
    );

    assert!(manager.snapshot(20, 1).is_none());
    assert_eq!(manager.snapshot(20, 2).unwrap().layer, Some(2));
    assert_eq!(manager.buff_id_amount(20, 109320111), 2);
}

#[test]
fn observed_single_copy_removes_a_duplicate_beside_the_known_uid() {
    let buff = |uid, layer| BuffInfo {
        uid: Some(uid),
        buff_id: Some(109320111),
        from_uid: Some(20),
        layer: Some(layer),
        ..Default::default()
    };
    let mut manager = manager_with_buffs(vec![buff(1, 1), buff(2, 1)]);

    assert!(manager.resync_observed_present(20, buff(2, 2)).unwrap());

    assert!(manager.snapshot(20, 1).is_none());
    assert_eq!(manager.snapshot(20, 2).unwrap().layer, Some(2));
    assert_eq!(manager.buff_id_amount(20, 109320111), 2);
}

#[test]
fn observed_stale_duplicate_reports_reconciliation() {
    let buff = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(109320111),
        from_uid: Some(20),
        layer: Some(1),
        ..Default::default()
    };
    let mut manager = manager_with_buffs(vec![buff(1), buff(2)]);

    assert!(manager.resync_observed_present(20, buff(2)).unwrap());

    assert!(manager.snapshot(20, 1).is_none());
    assert!(manager.snapshot(20, 2).is_some());
}

#[test]
fn observed_timed_layer_keeps_a_different_duration_copy() {
    let buff = |uid, duration| BuffInfo {
        uid: Some(uid),
        buff_id: Some(7111),
        from_uid: Some(20),
        layer: Some(1),
        duration: Some(duration),
        ..Default::default()
    };
    let mut manager = manager_with_buffs(vec![buff(1, 1), buff(2, 2)]);

    assert!(manager.resync_observed_present(20, buff(3, 1)).unwrap());

    assert!(manager.snapshot(20, 1).is_none());
    assert!(manager.snapshot(20, 2).is_some());
    assert!(manager.snapshot(20, 3).is_some());
}
