use super::*;

#[test]
fn rejects_unknown_and_ambiguous_include_policy() {
    assert_eq!(
        validate_include_entries(100, true, &[(99, 0)]),
        Err(BuffPolicyError::UnknownIncludeType {
            buff_id: 100,
            include_type: 99,
        })
    );
    assert_eq!(
        validate_include_entries(100, true, &[(10, 3), (11, 2)]),
        Err(BuffPolicyError::AmbiguousStorage(100))
    );
    assert_eq!(
        validate_include_entries(100, false, &[]),
        Err(BuffPolicyError::MalformedIncludeTypes(100))
    );
}

#[test]
fn reports_only_include_entries_without_runtime_semantics() {
    crate::test_support::init_config();
    let game = crate::test_support::game_data();

    assert_eq!(
        BuffPolicy::configured(game, 6200501),
        BuffPolicy::try_for_buff_id(6200501)
    );
    assert_eq!(
        BuffPolicy::configured(game, -1),
        Err(BuffPolicyError::MissingDefinition(-1))
    );

    assert!(
        BuffPolicy::try_for_buff_id(31170002)
            .unwrap()
            .unresolved_include_entries
            .is_empty()
    );
    let proven_shield_family = BuffPolicy::try_for_buff_id(31170002).unwrap();
    assert_eq!(
        proven_shield_family.match_existing,
        ExistingBuffMatch::SameIdAndDuration
    );
    let shared_type = BuffPolicy::try_for_buff_id(400301).unwrap();
    assert!(shared_type.unresolved_include_entries.is_empty());
    assert_eq!(
        shared_type.match_existing,
        ExistingBuffMatch::SharedTypeFamily
    );
    let exclusive_state = BuffPolicy::try_for_buff_id(500101).unwrap();
    assert!(exclusive_state.unresolved_include_entries.is_empty());
    assert_eq!(
        exclusive_state.match_existing,
        ExistingBuffMatch::SharedTypeFamily
    );
    assert_eq!(exclusive_state.on_duplicate, DuplicateGrant::KeepExisting);
    assert!(
        BuffPolicy::try_for_buff_id(31050145)
            .unwrap()
            .unresolved_include_entries
            .is_empty()
    );
    let same_type_capacity = BuffPolicy::try_for_buff_id(6200501).unwrap();
    assert!(same_type_capacity.unresolved_include_entries.is_empty());
    assert_eq!(same_type_capacity.same_type_capacity, Some(10));
    let beryl_count = BuffPolicy::try_for_buff_id(31130123).unwrap();
    assert!(beryl_count.unresolved_include_entries.is_empty());
    assert_eq!(beryl_count.storage, BuffStorage::Counted);
    assert_eq!(
        BuffPolicy::try_for_buff_id(31080111)
            .unwrap()
            .uid
            .normal_reservations_before_reapply,
        2
    );
    let tower_power = BuffPolicy::try_for_buff_id(130100112).unwrap();
    assert!(tower_power.unresolved_include_entries.is_empty());
    assert_eq!(tower_power.storage, BuffStorage::SeparateCopies);
    assert_eq!(
        tower_power.shared_group_capacity,
        Some(SharedGroupCapacity {
            group_id: 10,
            max_instances: 4,
        })
    );
    assert_eq!(
        BuffPolicy::try_for_buff_id(30950113)
            .unwrap()
            .uid
            .normal_reservations_after_first_apply,
        0
    );
}
