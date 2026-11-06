#[test]
fn target_shape_uses_definitions() {
    crate::init_test_config();
    let game = crate::get();
    assert_eq!(game.skill_target_type("1"), Some(1));
    assert_eq!(game.skill_target_type("202"), Some(2));
    assert_eq!(game.skill_target_type("202#1"), Some(2));
    assert_eq!(game.skill_target_type("103"), Some(1));
    assert_eq!(game.skill_target_type("0"), Some(0));
    assert_eq!(game.skill_target_type("missing"), None);
}
