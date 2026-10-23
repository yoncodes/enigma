use super::*;

#[test]
fn tracked_damage_records_the_indicator_immediately_after_hp() {
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
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.indicator.track_damage(
        crate::engine::manager::indicator::IndicatorId::BossRushScore,
        -1,
    );
    let result = run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "TestDamage"),
                },
                source_uid: 10,
                target_uid: -1,
                amount: 75,
                config_effect: 1,
                effect_kind: crate::engine::manager::hp::DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: crate::engine::manager::hp::HurtInfoData {
                    from_uid: 10,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 75,
                    effect_id: 1,
                    skill_id: 1,
                    damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                },
            }),
        ))],
    )
    .unwrap();

    let items = &result.frames[0].items;
    assert!(matches!(
        (&items[0], &items[1]),
        (FrameItem::Change(hp), FrameItem::Change(indicator))
            if matches!(hp.as_ref(), BattleChange::Hp(_))
                && matches!(indicator.as_ref(), BattleChange::EffectMarker(marker)
                    if marker.target_uid == 4 && marker.effect_num == 75)
    ));
}

#[test]
fn parent_owned_root_output_stays_in_the_root_frame() {
    use crate::engine::skill::behavior::registry::OutputOwner;

    assert_eq!(output_frame_path(OutputOwner::Parent, &[3]), vec![3]);
    assert_eq!(output_frame_path(OutputOwner::Parent, &[3, 4]), vec![3]);
    assert_eq!(output_frame_path(OutputOwner::Skill, &[3, 4]), vec![3, 4]);
}

#[test]
fn bloodtithe_spend_keeps_atomic_changes_in_their_semantic_frames() {
    use crate::engine::{
        manager::{
            buff::BuffGrant,
            gauge::{GaugeCommand, GaugeKey, GaugeKind, GaugeOperation, GaugeOwner},
        },
        mechanic::bloodtithe::spend::SpendCommand,
        runtime::record::FrameItem,
        skill::rule::SetupStage,
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60210, "ConsumeBloodAddBuff"),
    };
    let gauge_key = GaugeKey {
        kind: GaugeKind::Bloodtithe,
        owner: GaugeOwner::Team(1),
    };
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::Enable { max: Some(10) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::ChangeValue { delta: 1 },
        ))
        .unwrap();

    let trigger = FrameTrigger::Setup {
        stage: SetupStage::RoundStart,
        priority: 3,
    };
    let mut frames = Vec::new();
    let parent_path = push_root(&mut frames, FrameOwner::SetupMechanic, trigger.clone());
    let skill_path = push_child(
        &mut frames,
        &parent_path,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            card_index: -1,
            target_uid: Some(20),
        },
        trigger.clone(),
    );
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Command(BattleCommand::BloodtitheSpend(SpendCommand {
            gauge: GaugeCommand::new(origin, gauge_key, GaugeOperation::ChangeValue { delta: -1 }),
            buff: BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 20,
                buff_id: 31260151,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        })),
        trigger: SkillOpTrigger::Setup {
            stage: SetupStage::RoundStart,
            priority: 3,
        },
        skill_execution: None,
        frame_path: Some(skill_path.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31260181,
        slots: Vec::new(),
    });
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    assert!(matches!(
        result.events.as_slice(),
        [BattleEvent::GaugeChanged(_), BattleEvent::BuffAdded(_)]
    ));
    assert_eq!(result.frames[0].trigger, trigger);
    let FrameItem::Child(skill) = &result.frames[0].items[0] else {
        panic!("expected the pre-existing skill frame")
    };
    assert_eq!(skill.trigger, trigger);
    assert!(result.frames[0].items.iter().any(
        |item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Gauge(_)))
    ));
    assert!(skill.items.iter().any(
        |item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Buff(_)))
    ));
}

#[test]
fn one_skill_event_groups_all_of_its_subscribed_rules() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let subscriber = |slot_index, key| SkillSubscriber {
        owner_uid: 10,
        skill_id: 100,
        slot_index: Some(slot_index),
        key: SubscriptionKey::new(EventKind::SkillAction, key),
    };
    let op = || {
        RuleOp::Skill(
            SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into(),
        )
    };
    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![
                (subscriber(0, DefinitionKey::new(208, "None")), op()),
                (subscriber(1, DefinitionKey::new(210, "None")), op()),
            ],
            ..Default::default()
        },
        &BattleEvent::Kind(EventKind::SkillAction),
        Some(&[0]),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let [first, second] = queued.as_slice() else {
        panic!("expected two subscribed rules")
    };
    assert!(Rc::ptr_eq(
        first.frame_group.as_ref().unwrap(),
        second.frame_group.as_ref().unwrap(),
    ));
}

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

#[test]
fn death_outside_the_subscriber_group_supersedes_its_pending_transition() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
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
    let catalog = SkillEffectCatalog::default();
    let mut determinism = RoundDeterminism::default();
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let queued = |op, subscriber: Option<SubscriberContext>| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber,
        caster_frame: None,
    };
    let kill = |source_uid, opcode, name| {
        RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(opcode, name),
                },
                source_uid,
                target_uid: -1,
                config_effect: 1,
            }),
        ))
    };
    let outer_group = Rc::new(RefCell::new(SubscriberDeathGroup::default()));
    let outer = SubscriberContext {
        owner_uid: -1,
        skill_id: None,
        group: Some(outer_group),
    };
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(kill(10, 701, "OuterDamage"), Some(outer.clone()))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [
            queued(
                RuleOp::Command(BattleCommand::Hp(
                    crate::engine::manager::hp::HpCommand::Heal(
                        crate::engine::manager::hp::HpHeal {
                            origin: CommandOrigin {
                                domain: RuleDomain::BuffAct,
                                key: DefinitionKey::new(702, "NestedRevive"),
                            },
                            source_uid: 11,
                            target_uid: -1,
                            amount: 1,
                            config_effect: 1,
                            kind: crate::engine::manager::hp::HpHealKind::Revive,
                        },
                    ),
                )),
                None,
            ),
            queued(kill(11, 703, "NestedDamage"), None),
        ]
        .into(),
        &mut result,
        &mut state,
    )
    .unwrap();
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(RuleOp::SubscriberGroupComplete, Some(outer))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::DeathSettlement(death) => Some(*death),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [crate::engine::manager::hp::DeathTransition {
            source_uid: 11,
            target_uid: -1,
        }]
    );
}

#[test]
fn subscriber_death_batch_cleans_each_defeated_halo_emitter() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11, 30, 12]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    for target_uid in [10, 11, 30, 12] {
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 30,
                target_uid,
                buff_id: 31450141,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
    }

    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let group = Rc::new(RefCell::new(SubscriberDeathGroup::default()));
    let subscriber = SubscriberContext {
        owner_uid: 10,
        skill_id: None,
        group: Some(group),
    };
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(subscriber.clone()),
        caster_frame: None,
    };
    let kill = |target_uid| {
        RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestBatchDeath"),
                },
                source_uid: -1,
                target_uid,
                config_effect: 1,
            }),
        ))
    };
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut determinism = RoundDeterminism::default();
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(kill(11)), queued(kill(30))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(RuleOp::SubscriberGroupComplete)].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    for owner_uid in [10, 12] {
        assert_eq!(
            managers
                .buff
                .active_for(owner_uid)
                .filter(|buff| buff.buff_id == Some(31450143))
                .count(),
            2
        );
        assert_eq!(managers.buff.buff_id_amount(owner_uid, 31450143), 1);
    }
}

#[test]
fn deferred_same_owner_batches_share_one_final_completion() {
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::SkillAction,
    ));
    let queued = |buff_act_id| QueuedOp {
        op: RuleOp::BuffFeatureMarker {
            target_uid: 10,
            effect_type: 357,
            effect_num: 1,
            buff_act_id,
        },
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(10)),
        caster_frame: None,
    };
    let action_path = vec![0];
    let batch = |buff_act_id| {
        let mut reactions = ReactionBatch {
            after_hit_skills: vec![queued(buff_act_id)],
            ..Default::default()
        };
        reactions.assign_subscriber_death_groups(Some(&action_path));
        reactions.after_hit_skills
    };
    let mut state = DrainState::new(TargetContext::default());
    state.defer_after_hit_skills(Some(&action_path), batch(701));
    state.defer_after_hit_skills(Some(&action_path), batch(702));
    let queued = state.take_after_hit_skills(Some(&action_path)).unwrap();

    assert_eq!(
        queued
            .iter()
            .filter(|queued| matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        queued.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
}
