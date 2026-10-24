use super::configured_battle;

#[test]
fn explicit_battle_identity_never_falls_back_to_the_episode() {
    crate::test_support::init_config();

    let invalid = sonettobuf::Fight {
        episode_id: Some(10103),
        battle_id: Some(i32::MAX),
        ..Default::default()
    };
    assert!(configured_battle(&invalid).is_none());

    let episode_only = sonettobuf::Fight {
        episode_id: Some(10103),
        ..Default::default()
    };
    assert_eq!(
        configured_battle(&episode_only).map(|battle| battle.id),
        Some(1102)
    );
}
