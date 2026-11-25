use std::collections::VecDeque;

use super::*;
use crate::engine::{
    event::payload::FieldChangeEvent,
    manager::field::FieldChangeKind,
    runtime::{
        drain::{DrainResult, DrainState, QueuedOp, SubscriberContext, drain_nested_queue},
        record::{push_child, push_root},
        skill::SkillOpTrigger,
    },
    skill::{
        condition::registry::ADD_MAGIC_CIRCLE_KEY,
        rule::{DefinitionKey, output::RuleOp},
    },
};

#[test]
fn field_reward_waits_for_grant_even_when_the_gain_uses_another_frame() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![31350164],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let level = BattleEvent::FieldChanged(FieldChangeEvent {
        origin,
        team: 1,
        kind: FieldChangeKind::Level,
        field_id: 30003,
        before_level: 1,
        after_level: 3,
        before_progress: 20,
        after_progress: 0,
        overflow: 0,
    });

    for (trigger, waits, gain_uses_separate_frame) in [
        (SkillOpTrigger::Event(level.clone()), true, true),
        (SkillOpTrigger::Event(level.clone()), true, false),
        (SkillOpTrigger::Active, false, true),
    ] {
        let mut result = DrainResult::default();
        let owner = push_root(
            &mut result.frames,
            FrameOwner::Command,
            FrameTrigger::Active,
        );
        let grant_frame = push_child(
            &mut result.frames,
            &owner,
            FrameOwner::Command,
            FrameTrigger::Event(level.clone()),
        );
        let output_frame = push_root(
            &mut result.frames,
            FrameOwner::Command,
            FrameTrigger::Active,
        );
        let mut subscriber = SubscriberContext::for_skill(10, 31350161);
        subscriber.field_level =
            super::super::grant_scope(ADD_MAGIC_CIRCLE_KEY, &level, Some(&owner));
        assert!(super::super::grant_scope(origin.key, &level, Some(&owner)).is_none());
        super::super::start_grant(&mut subscriber.field_level, &grant_frame);
        let gain_frame = if gain_uses_separate_frame {
            output_frame.clone()
        } else {
            grant_frame
        };

        let command = |buff_id, amount, trigger, frame_path, subscriber| QueuedOp {
            op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id,
                amount,
                occurrences: 1,
                child_uid_reservations: 0,
            }))),
            trigger,
            skill_execution: None,
            frame_path: Some(frame_path),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber,
            caster_frame: None,
        };
        let queue = VecDeque::from([
            command(
                31350131,
                Some(15),
                trigger,
                gain_frame,
                Some(subscriber.clone()),
            ),
            command(
                31350171,
                None,
                SkillOpTrigger::Event(level.clone()),
                output_frame,
                Some(subscriber),
            ),
        ]);
        let mut managers = BattleManagers::seeded(&fight);
        drain_nested_queue(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            queue,
            &mut result,
            &mut DrainState::new(Default::default()),
        )
        .unwrap();
        let added = result.outcomes.iter().filter_map(|outcome| match outcome {
            RuleOutcome::Buff(outcome) => outcome.change.added.as_ref()?.buff.buff_id,
            _ => None,
        });
        let order: Vec<_> = added
            .filter(|id| matches!(id, 31350171 | 31350201))
            .collect();
        let expected = if waits {
            [31350171, 31350201]
        } else {
            [31350201, 31350171]
        };
        assert_eq!(order, expected);
    }
}
