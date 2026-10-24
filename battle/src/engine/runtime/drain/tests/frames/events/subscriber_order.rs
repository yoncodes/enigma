use super::*;

#[test]
fn independent_reaction_does_not_inherit_the_causing_frame() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![(
                SkillSubscriber {
                    owner_uid: -1,
                    skill_id: 100,
                    slot_index: Some(0),
                    key: SubscriptionKey::new(
                        EventKind::RoundEndAfterOwnerSettlement,
                        DefinitionKey::new(305, "None"),
                    ),
                },
                RuleOp::Skill(
                    SkillRequest {
                        source_uid: -1,
                        skill_id: 100,
                    }
                    .into(),
                ),
            )],
            ..Default::default()
        },
        &BattleEvent::Kind(EventKind::RoundEndAfterOwnerSettlement),
        Some(&[4, 2]),
        Some(&[4, 2]),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let [reaction] = queued.as_slice() else {
        panic!("expected one reaction")
    };
    assert!(reaction.frame_path.is_none());
    assert!(reaction.parent_path.is_none());
    assert!(reaction.independent_parent_group.is_some());
}

#[test]
fn queued_reaction_rejects_an_unregistered_exact_condition() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::{
            rule::route::RouteError,
            subscriber::{SkillSubscriber, SubscriberError},
        },
    };

    let result = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![(
                SkillSubscriber {
                    owner_uid: 10,
                    skill_id: 100,
                    slot_index: Some(0),
                    key: SubscriptionKey::new(
                        EventKind::SkillAction,
                        DefinitionKey::new(999_999, "Unknown"),
                    ),
                },
                RuleOp::Skill(
                    SkillRequest {
                        source_uid: 10,
                        skill_id: 100,
                    }
                    .into(),
                ),
            )],
            ..Default::default()
        },
        &BattleEvent::Kind(EventKind::SkillAction),
        Some(&[0]),
        None,
        None,
        None,
        None,
        None,
    );

    assert!(matches!(
        result,
        Err(DrainError::Subscriber(SubscriberError::UncompiledRoute {
            skill_id: 100,
            route: RouteError::UnregisteredExactKey {
                opcode: 999_999,
                ..
            },
        }))
    ));
}

#[test]
fn committed_event_runs_its_subscriber_before_the_queue_continues() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![100],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(1),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60189, "AddEnergyToCard"),
            vec![1, 2, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 721017,
        type_name: "CurEntityPowerDel".to_owned(),
        kind: ParsedConditionKind::CurrentEntityPowerDecrease,
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                origin,
                source_uid: 10,
                target_uid: 10,
                power_id: EUREKA_RESOURCE_ID,
                delta: -1,
                effect_type: 0,
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { origin })),
        ],
    )
    .unwrap();

    assert!(matches!(
        result.outcomes.as_slice(),
        [
            RuleOutcome::Eureka(_),
            RuleOutcome::Card(reaction),
            RuleOutcome::Card(queued)
        ] if reaction.kind == CardChangeKind::EnergyChanged
            && queued.kind == CardChangeKind::EnergyCleared
    ));
    assert!(matches!(
        result.events.as_slice(),
        [BattleEvent::EurekaChanged(_)]
    ));
    assert_eq!(result.frames.len(), 2);
    let [
        crate::engine::runtime::record::FrameItem::Change(eureka),
        crate::engine::runtime::record::FrameItem::Child(child),
    ] = result.frames[0].items.as_slice()
    else {
        panic!("expected committed change followed by its subscriber frame")
    };
    assert!(matches!(eureka.as_ref(), BattleChange::Eureka(_)));
    assert!(matches!(
        child.owner,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            target_uid: Some(10),
            ..
        }
    ));
    assert!(matches!(
        child.items.as_slice(),
        [crate::engine::runtime::record::FrameItem::Change(change)]
            if matches!(change.as_ref(), BattleChange::Card(_))
    ));
    assert!(matches!(
        result.frames[1].items.as_slice(),
        [crate::engine::runtime::record::FrameItem::Change(change)]
            if matches!(change.as_ref(), BattleChange::Card(_))
    ));
}

#[test]
fn lifecycle_event_enters_the_same_subscriber_drain() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 208,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillAction),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let event = BattleEvent::Kind(crate::engine::event::kind::EventKind::SkillAction);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(matches!(
        result.events.as_slice(),
        [
            BattleEvent::Kind(crate::engine::event::kind::EventKind::SkillAction),
            BattleEvent::ExPointChanged(_)
        ]
    ));
    assert!(matches!(
        result.outcomes.as_slice(),
        [RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
            change,
            ..
        })] if change.applied_delta == 1
    ));

    managers
        .hp
        .execute_command(crate::engine::manager::hp::HpCommand::Kill(
            crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60019, "KillTargets"),
                },
                source_uid: 10,
                target_uid: 10,
                config_effect: 60019,
            },
        ))
        .unwrap();

    let after_death = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(crate::engine::event::kind::EventKind::SkillAction),
    )
    .unwrap();

    assert!(after_death.outcomes.is_empty());
    assert_eq!(managers.ex_point.get(10), 1);
}

#[test]
fn unscoped_deferred_lane_does_not_strand_death_settlement() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(-1)),
        caster_frame: None,
    };
    let mut reactions = ReactionBatch {
        after_publish: vec![queued(RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(701, "TestDamage"),
                },
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            }),
        )))],
        after_hit_skills: vec![queued(RuleOp::BuffFeatureMarker {
            target_uid: -1,
            effect_type: 357,
            effect_num: 1,
            buff_act_id: 702,
        })],
        ..Default::default()
    };
    reactions.assign_subscriber_death_groups(None);
    assert!(matches!(
        reactions.after_publish.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
    assert!(
        !reactions
            .after_hit_skills
            .iter()
            .any(|queued| matches!(queued.op, RuleOp::SubscriberGroupComplete))
    );

    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut reactions.into_ordered().into(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(
        result
            .outcomes
            .iter()
            .filter(|outcome| matches!(outcome, RuleOutcome::DeathSettlement(_)))
            .count(),
        1
    );
}
