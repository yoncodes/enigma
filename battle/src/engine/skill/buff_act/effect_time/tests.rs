use std::collections::HashSet;

use super::*;

#[test]
fn keeps_round_end_channels_separate() {
    assert_eq!(classify(302), BuffActEvent::Runtime(EventKind::RoundEnd));
    assert_eq!(
        classify(303),
        BuffActEvent::Runtime(EventKind::RoundEndEntitySettlement)
    );
    assert_eq!(
        classify(304),
        BuffActEvent::Runtime(EventKind::RoundEndAfterSettlement)
    );
    assert_eq!(
        classify(305),
        BuffActEvent::Runtime(EventKind::ExPointOverflow)
    );
    assert_eq!(
        classify(307),
        BuffActEvent::Runtime(EventKind::RoundEndFinalSettlement)
    );
}

#[test]
fn keeps_non_event_channels_out_of_runtime_events() {
    assert_eq!(classify(202), BuffActEvent::DamageCalculation);
    assert_eq!(classify(-1), BuffActEvent::CycleSubstitution);
    assert_eq!(classify(1051), BuffActEvent::CardChoice);
}

#[test]
fn exact_effect_time_definitions_are_unique() {
    let definitions = definitions().collect::<Vec<_>>();
    let unique = definitions
        .iter()
        .map(|definition| definition.key.opcode)
        .collect::<HashSet<_>>();

    assert_eq!(definitions.len(), unique.len());
}

#[test]
fn small_round_end_keeps_each_configured_duration_stage() {
    assert_eq!(
        duration_stages_for_event(EventKind::SmallRoundEnd).collect::<Vec<_>>(),
        vec![211, 301]
    );
}

#[test]
fn before_ap_resolution_owns_action_queue_duration() {
    assert_eq!(
        classify(107),
        BuffActEvent::Runtime(EventKind::ActionQueueCommitted)
    );
    assert_eq!(
        duration_stages_for_event(EventKind::ActionQueueCommitted).collect::<Vec<_>>(),
        vec![107]
    );
}

#[test]
fn duration_support_accepts_non_advancing_and_scheduled_policies() {
    assert!(supports_duration_policy(-1));
    assert!(supports_duration_policy(ROUND_START_INITIAL_DURATION));
    assert!(supports_duration_policy(ROUND_START_ACTOR_DURATION));
    assert!(supports_duration_policy(
        ROUND_START_BEFORE_CONDITION_DURATION
    ));
    assert!(supports_duration_policy(ROUND_START_DURATION));
    assert!(supports_duration_policy(
        ROUND_START_AFTER_REACTION_DURATION
    ));
    assert!(supports_duration_policy(210));
    assert!(supports_duration_policy(107));
    assert!(supports_duration_policy(212));
    assert!(supports_duration_policy(301));
    assert!(supports_duration_policy(ROUND_END_ENTITY_SETTLEMENT));
    assert!(supports_duration_policy(ROUND_END_AFTER_SETTLEMENT));
    assert!(supports_duration_policy(ROUND_END_FINAL_SETTLEMENT));
    assert!(!supports_duration_policy(209));
    assert!(!supports_duration_policy(205));
}

#[test]
fn after_hit_duration_advances_when_hit_passives_complete() {
    assert_eq!(
        duration_stage_after_skill_phase(SkillPhase::HitPassives),
        Some(210)
    );
    assert_eq!(duration_stage_after_skill_phase(SkillPhase::AfterHit), None);
}

#[test]
fn device_actions_use_the_registered_duration_phase() {
    assert_eq!(
        duration_stage_after_skill_phase(SkillPhase::HitPassives),
        Some(210)
    );
    assert_eq!(duration_stage_after_skill_phase(SkillPhase::AfterHit), None);
}
