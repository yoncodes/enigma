use super::*;
use crate::engine::{
    event::payload::ExPointChangeEvent,
    manager::buff::BuffBankExPointOverflow,
    runtime::record::{push_child, push_root},
    skill::rule::CommandOrigin,
};

#[test]
fn only_a_self_owned_round_end_bank_releases_the_skill_frame() {
    let owner = 10;
    let round_end = FrameTrigger::Event(BattleEvent::Kind(EventKind::RoundEnd));
    let mut frames = Vec::new();
    let phase = push_root(&mut frames, FrameOwner::EventRule, round_end.clone());
    let skill = FrameOwner::Skill {
        source_uid: owner,
        target_uid: Some(owner),
        skill_id: 1,
        card_index: 0,
    };
    let phase_skill = push_child(&mut frames, &phase, skill.clone(), round_end);
    let active_skill = push_root(&mut frames, skill, FrameTrigger::Active);
    let bank = RuleOp::Command(BattleCommand::Buff(BuffCommand::BankExPointOverflow(
        BuffBankExPointOverflow {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(806, "ExPointOverflowBank"),
            },
            target_uid: owner,
            buff_uid: 20,
            overflow: 1,
            capacity: 2,
        },
    )));
    let overflow = |source, applied_delta| {
        SkillOpTrigger::Event(BattleEvent::ExPointOverflow(ExPointChangeEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(20002, "AddExPoint"),
            },
            source_uid: source,
            target_uid: owner,
            kind: ExPointKind::Common,
            before: 5 - applied_delta,
            requested_delta: 1 + applied_delta,
            applied_delta,
            after: 5,
            overflow: 1,
        }))
    };
    assert_eq!(
        storage_scope(&frames, Some(&phase_skill), &overflow(owner, 0), &bank),
        Some(phase.as_slice())
    );
    for (path, trigger) in [
        (&active_skill, overflow(owner, 0)),
        (&phase_skill, overflow(owner, 1)),
        (&phase_skill, overflow(11, 0)),
    ] {
        assert_eq!(storage_scope(&frames, Some(path), &trigger, &bank), None);
    }
}
