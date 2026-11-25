use super::*;
use crate::engine::skill::rule::DefinitionKey;

const RULE: ConfiguredRuleKey =
    ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill"));

#[test]
fn completed_cast_progress_resets_at_its_next_phase_but_untouched_progress_carries() {
    let advance = |manager: &mut ConfiguredRuleProgressManager, amount| {
        manager.advance_until_cast(10, RULE, 7, amount)
    };

    let mut completed = ConfiguredRuleProgressManager::default();
    assert!(!advance(&mut completed, 6));
    completed.begin_round_start();
    assert!(advance(&mut completed, 1));
    completed.release(10, RULE);
    assert!(!advance(&mut completed, 2));
    completed.begin_action_round();
    assert!(!advance(&mut completed, 6));
    assert!(advance(&mut completed, 1));

    let mut untouched = ConfiguredRuleProgressManager::default();
    assert!(!advance(&mut untouched, 6));
    untouched.begin_round_start();
    untouched.begin_action_round();
    assert!(advance(&mut untouched, 1));

    let mut pending = ConfiguredRuleProgressManager::default();
    assert!(!advance(&mut pending, 6));
    pending.begin_round_start();
    assert!(advance(&mut pending, 1));
    pending.begin_action_round();
    assert!(!advance(&mut pending, 7));
    pending.release(10, RULE);
    assert!(!advance(&mut pending, 6));
    assert!(advance(&mut pending, 1));

    let mut cloth = ConfiguredRuleProgressManager::default();
    cloth.begin_action_round();
    assert!(advance(&mut cloth, 7));
    cloth.release(10, RULE);
    assert!(!advance(&mut cloth, 2));
    cloth.begin_round_start();
    cloth.begin_cloth_sequence();
    assert!(!advance(&mut cloth, 6));
    cloth.begin_cloth_sequence();
    assert!(advance(&mut cloth, 1));
    cloth.release(10, RULE);
    assert!(!advance(&mut cloth, 2));
    cloth.begin_action_round();
    assert!(!advance(&mut cloth, 4));
    assert!(advance(&mut cloth, 1));
}

#[test]
fn completed_cast_progress_clears_when_new_round_start_follows_settlement() {
    let mut manager = ConfiguredRuleProgressManager::default();
    manager.begin_round_start();
    assert!(manager.advance_until_cast(10, RULE, 7, 7));
    manager.release(10, RULE);
    assert!(!manager.advance_until_cast(10, RULE, 7, 2));

    manager.begin_round_start();
    assert!(!manager.advance_until_cast(10, RULE, 7, 6));
    assert!(manager.advance_until_cast(10, RULE, 7, 1));
}
