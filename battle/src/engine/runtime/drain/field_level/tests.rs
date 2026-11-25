use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        field::{FieldCommand, FieldDefinition, FieldOperation, FieldThreshold},
    },
    runtime::{
        determinism::RoundDeterminism,
        drain,
        executor::RuleOutcome,
        record::{FrameItem, FrameOwner, FrameTrigger},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
        target::TargetPool,
    },
};

mod separate_frame;

#[test]
fn recipient_reward_follows_the_field_level_skill() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    passive_skill: vec![31350161, 31350164],
                    buffs: vec![BuffInfo {
                        uid: Some(100),
                        buff_id: Some(31350171),
                        from_uid: Some(10),
                        duration: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut precognition = catalog.get(31350161).unwrap().clone();
    let slot = precognition.slots[2].clone();
    precognition.slots.splice(
        2..3,
        [10, 5, 5, 10].map(|amount| {
            let mut slot = slot.clone();
            slot.behavior.args[1] = amount;
            slot.behavior.raw_args[1] = amount.to_string();
            slot
        }),
    );
    catalog.insert(precognition);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut run = |managers: &mut BattleManagers, command: BattleCommand| {
        drain::run_command_group(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            Default::default(),
            [super::RuleOp::Command(command)],
        )
        .unwrap()
    };

    let grant = |target_uid, amount| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid,
            buff_id: 31350131,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    run(&mut managers, BattleCommand::Buff(grant(11, 15)));
    assert_eq!(managers.buff.buff_id_or_type_amount(11, 31350131), 15);
    assert!(!managers.buff.has_buff_id(10, 31350201));

    let mut batch_managers = BattleManagers::seeded(&fight);
    let batch = run(
        &mut batch_managers,
        BattleCommand::BuffBatch(vec![grant(10, 10), grant(10, 5)]),
    );
    let applied = batch.events.iter().filter_map(|event| match event {
        BattleEvent::BuffAdded(gain) | BattleEvent::BuffChanged(gain)
            if gain.buff_id == 31350131 =>
        {
            gain.added_type_total
        }
        _ => None,
    });
    assert!(applied.eq([10, 15]));
    assert_eq!(batch_managers.buff.buff_id_or_type_amount(10, 90071), 5);
    assert_eq!(batch_managers.ex_point.get(10), 1);

    let field = |operation| FieldCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(50019, "AddMagicCircle"),
        },
        team: 1,
        operation,
    };
    let thresholds = vec![FieldThreshold {
        level: 3,
        progress: 20,
        definition: FieldDefinition {
            field_id: 30003,
            duration: 3,
        },
    }];
    managers
        .execute_field(field(FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: 30001,
                duration: 3,
            },
            create_uid: 10,
            initial_level: 1,
            thresholds: thresholds.clone(),
        }))
        .unwrap();
    managers
        .execute_field(field(FieldOperation::ChangeProgress { delta: 20 }))
        .unwrap();

    let result = run(
        &mut managers,
        BattleCommand::Field(field(FieldOperation::ResolveLevel { thresholds })),
    );
    assert_eq!(managers.buff.buff_id_or_type_amount(10, 31350131), 30);
    assert_eq!(managers.buff.buff_id_or_type_amount(10, 90071), 10);
    assert_eq!(managers.ex_point.get(10), 2);
    let discharge = result.outcomes.iter().position(
        |outcome| matches!(outcome, RuleOutcome::Buff(changes)
            if changes.change.refreshed.iter().any(|update| update.after.buff_id == Some(31350171) && update.after.duration == Some(3))),
    );
    let marker = result.outcomes.iter().position(
        |outcome| matches!(outcome, RuleOutcome::Buff(changes)
            if changes.change.added.as_ref().is_some_and(|added| added.buff.buff_id == Some(31350201))),
    );
    assert!(
        discharge.unwrap() < marker.unwrap(),
        "{discharge:?} {marker:?}"
    );

    let skills = result.frames[0]
        .items
        .iter()
        .filter_map(|item| {
            let FrameItem::Child(frame) = item else {
                return None;
            };
            let FrameOwner::Skill { skill_id, .. } = frame.owner else {
                return None;
            };
            if frame.items.is_empty() {
                return None;
            }
            let gain = match &frame.trigger {
                FrameTrigger::Event(BattleEvent::BuffChanged(change)) => Some(change.after_amount),
                _ => None,
            };
            Some((skill_id, gain))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        skills,
        [
            (31350161, None),
            (31350164, Some(15)),
            (31350164, None),
            (31350164, Some(30)),
            (31350164, None),
        ]
    );
}

#[test]
fn earlier_reward_group_finishes_before_the_next_reward() {
    use std::collections::VecDeque;

    use super::super::{
        DrainResult, DrainState, QueuedOp, ReactionBatch, SubscriberContext, drain_nested_queue,
    };
    use crate::engine::{
        event::payload::{BuffChangeEvent, BuffExpiry},
        manager::hp::{HpCommand, HpKill},
        runtime::record::{push_child, push_root},
        skill::action::{SkillInvocation, SkillRequest},
    };

    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![31350164],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut result = DrainResult::default();
    let parent = push_root(
        &mut result.frames,
        FrameOwner::Command,
        FrameTrigger::Active,
    );
    let other = push_root(
        &mut result.frames,
        FrameOwner::Command,
        FrameTrigger::Active,
    );
    let field_skill = push_child(
        &mut result.frames,
        &other,
        FrameOwner::Command,
        FrameTrigger::Active,
    );
    let grant = super::Grant {
        owner: parent.clone(),
        completion: field_skill.clone(),
    };
    let reward_batch = |total| {
        let invocation = SkillInvocation {
            condition_key: Some(DefinitionKey::new(
                730004,
                "AccAddBuffCountByBuffTypeIdAllEffect",
            )),
            condition_slot: Some(0),
            ..SkillRequest {
                source_uid: 10,
                skill_id: 31350164,
            }
            .into()
        };
        let mut reactions = ReactionBatch {
            after_publish: vec![QueuedOp {
                op: super::RuleOp::Skill(invocation),
                trigger: super::SkillOpTrigger::Event(BattleEvent::BuffChanged(BuffChangeEvent {
                    source_uid: 10,
                    target_uid: 10,
                    buff_uid: 100,
                    buff_id: 31350131,
                    before_amount: total - 5,
                    after_amount: total,
                    added_type_total: Some(total),
                    act_id: 0,
                    act_value: 0,
                    replaced_fixed_max_hp: None,
                    replaced_buff_id: None,
                    expiry: BuffExpiry::None,
                })),
                skill_execution: None,
                frame_path: None,
                parent_path: Some(field_skill.clone()),
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber: Some(SubscriberContext::for_skill(10, 31350164)),
                caster_frame: None,
            }],
            ..Default::default()
        };
        super::bind_reactions(Some(&grant), None, &mut reactions);
        assert!(reactions.after_publish.is_empty());
        assert_eq!(reactions.after_skill[0].parent_path.as_ref(), Some(&parent));
        reactions.after_skill
    };
    let mut state = DrainState::new(Default::default());
    let death = managers
        .execute_hp(HpCommand::Kill(HpKill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "TestKill"),
            },
            source_uid: 10,
            target_uid: -1,
            config_effect: 1,
        }))
        .unwrap()
        .death
        .unwrap();
    let first = reward_batch(15);
    let group = first[0]
        .subscriber
        .as_ref()
        .unwrap()
        .group
        .as_ref()
        .unwrap();
    state.record_subscriber_deaths(group, &field_skill, [death]);
    let mut queue = VecDeque::new();
    super::schedule_after_skill(&mut queue, &other, Some(&grant), first);
    super::schedule_after_skill(&mut queue, &other, Some(&grant), reward_batch(30));

    drain_nested_queue(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &SkillEffectCatalog::from_game_db(config::configs::get()),
        &mut RoundDeterminism::default(),
        queue,
        &mut result,
        &mut state,
    )
    .unwrap();
    let order = result.outcomes.iter().filter_map(|outcome| match outcome {
        RuleOutcome::Buff(changes)
            if changes
                .change
                .added
                .as_ref()
                .is_some_and(|added| added.buff.buff_id == Some(31350201)) =>
        {
            Some("reward")
        }
        RuleOutcome::DeathSettlement(_) => Some("death"),
        _ => None,
    });
    assert!(order.eq(["reward", "death", "reward"]));
}
