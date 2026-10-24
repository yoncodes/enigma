use super::*;

#[test]
fn total_and_round_rule_limits_have_distinct_lifetimes() {
    let mut managers = BattleManagers::default();
    let owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(0);

    assert!(managers.can_fire_rule(1, 433011, owner, 1, 0));
    managers.mark_rule_fired(1, 433011, owner);
    assert!(!managers.can_fire_rule(1, 433011, owner, 1, 0));
    managers.begin_round();
    assert!(!managers.can_fire_rule(1, 433011, owner, 1, 0));

    let round_owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(1);
    assert!(managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    managers.mark_rule_fired(1, 433011, round_owner);
    assert!(!managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    managers.begin_round();
    assert!(managers.can_fire_rule(1, 433011, round_owner, 0, 1));
    assert!(managers.can_fire_rule(
        1,
        433011,
        crate::engine::skill::effect::RuleLimitOwner::from_slot(2),
        0,
        0
    ));
}

#[test]
fn action_round_state_preserves_next_round_reaction_usage() {
    let mut managers = BattleManagers::default();
    let owner = crate::engine::skill::effect::RuleLimitOwner::from_slot(0);

    managers.mark_rule_fired(-1, 100, owner);
    managers.begin_reaction_round();
    assert!(managers.can_fire_rule(-1, 100, owner, 0, 1));

    managers.mark_rule_fired(-1, 100, owner);
    managers.begin_action_round();
    assert!(!managers.can_fire_rule(-1, 100, owner, 0, 1));
}

#[test]
fn rule_progress_ignores_non_positive_movement() {
    let mut managers = BattleManagers::default();
    let key = DefinitionKey::new(1, "Progress");

    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, -1), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 5), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 1), 1);
}

#[test]
fn rule_progress_is_owned_by_the_exact_listener_instance() {
    let mut managers = BattleManagers::default();
    let key = DefinitionKey::new(1, "Progress");

    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 5), 0);
    assert_eq!(managers.advance_rule_progress(10, 21, key, 6, 1), 0);
    assert_eq!(managers.advance_rule_progress(10, 20, key, 6, 1), 1);
}
