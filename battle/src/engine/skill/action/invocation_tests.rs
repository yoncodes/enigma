use super::*;
use crate::engine::skill::condition::extra::ExtraSkillKind;

#[test]
fn invocation_keeps_runtime_target_and_action_kind() {
    let invocation = SkillInvocation {
        plan: SkillRequest {
            source_uid: 10,
            skill_id: 20,
        },
        card_index: 4,
        additional_moxie: 0,
        moxie_spent: 0,
        paid_ultimate: None,
        condition_key: Some(DefinitionKey::new(30, "TestCondition")),
        reaction_multiplicity_key: None,
        condition_slot: Some(2),
        field_level_settlement: false,
        reaction_batch_id: None,
        reaction_action_path: None,
        phase: Some(SkillPhase::AfterDamage),
        target: SkillTarget::Explicit(-1),
        frame_target: None,
        extra_skill_kind: Some(ExtraSkillKind::FollowUp),
        target_observed_extra_action: None,
        rate_modifier: None,
        trigger: SkillTrigger::Explicit(30),
        mode: SkillExecutionMode::Nested,
        start: SkillStart::Immediate,
        release_progress: None,
        action_hand_settlement: None,
        card_enchants: Vec::new(),
        recorded_skill: None,
        round_cap: None,
    };

    assert_eq!(invocation.target, SkillTarget::Explicit(-1));
    assert_eq!(
        invocation.condition_key,
        Some(DefinitionKey::new(30, "TestCondition"))
    );
    assert_eq!(invocation.card_index, 4);
    assert_eq!(invocation.condition_slot, Some(2));
    assert_eq!(invocation.phase, Some(SkillPhase::AfterDamage));
    assert_eq!(invocation.extra_skill_kind.unwrap().id(), 2);
    assert_eq!(invocation.trigger, SkillTrigger::Explicit(30));
}

#[test]
fn plain_configured_skill_does_not_inherit_runtime_context() {
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 20,
    }
    .into();

    assert_eq!(invocation.target, SkillTarget::Configured);
    assert_eq!(invocation.trigger, SkillTrigger::None);
}
