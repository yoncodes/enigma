use super::*;
use crate::engine::manager::hp::HpManager;

#[test]
fn weaker_family_grant_is_discarded_without_prolonging_or_reactivation() {
    crate::test_support::init_config();
    let hp = HpManager::default();
    for (stronger, weaker) in [(30880151, 30880150), (712314, 120380010)] {
        let mut manager = BuffManager::default();
        manager.seed(&sonettobuf::Fight {
            version: Some(7),
            ..Default::default()
        });
        let held = manager
            .add_replacing_excluded(&hp, 10, 10, stronger, 0)
            .added
            .unwrap();
        manager.advance_durations(103);
        let before = manager.snapshot(10, held.buff.uid.unwrap()).unwrap();
        let discarded = manager.add_replacing_excluded(&hp, 20, 10, weaker, 0);
        assert!(discarded.added.is_none());
        assert!(discarded.removed.is_empty());
        assert!(discarded.refreshed.is_empty());
        assert_eq!(manager.snapshot(10, held.buff.uid.unwrap()), Some(before));
        assert_eq!(manager.buffs.len(), 1);
        let mut next_grant = manager.clone();
        let next = next_grant
            .add_replacing_excluded(&hp, 20, 30, weaker, 0)
            .added
            .unwrap();
        assert_eq!(next.buff.uid, held.buff.uid.map(|uid| uid + 4));
        manager.advance_durations(103);
        manager.advance_durations(103);
        assert!(manager.active_for(10).next().is_none());

        let fresh = manager
            .add_replacing_excluded(&hp, 20, 10, weaker, 0)
            .added
            .unwrap();
        assert_ne!(fresh.buff.uid, held.buff.uid);
        let equal = manager.add_replacing_excluded(&hp, 30, 10, weaker, 0);
        assert_eq!(equal.removed.len(), 1);
        assert_ne!(equal.added.unwrap().buff.uid, fresh.buff.uid);
    }
}
