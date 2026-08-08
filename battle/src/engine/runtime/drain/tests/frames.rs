use super::*;
use crate::engine::runtime::record::FrameItem;

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
fn owner_event_runs_only_the_selected_side() {
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: vec![100],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
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

    let result = run_owner_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(crate::engine::event::kind::EventKind::SkillAction),
        &[10],
    )
    .unwrap();

    assert_eq!(result.outcomes.len(), 1);
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(-1), 0);
}

#[test]
fn active_skill_commits_immediate_ops_and_keeps_cast_state() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1000),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri_dmg: Some(1000),
            ..Default::default()
        },
    );
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(60189, "AddEnergyToCard"),
                    vec![1, 2, 1],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(10004, "AttrFix"),
                    vec![AttrId::CriticalDmg as i32, 500],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30015, "OriginDamageCanCrit"),
                    vec![0, AttrId::CurrentHp as i32, 1000],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
        ],
    });

    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(100, 10, [true]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Card(card) if card.kind == CardChangeKind::EnergyChanged
    )));
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Hp(execution)
            if execution
                .changes
                .hp
                .is_some_and(|change| change.delta == -150)
    )));
}

#[test]
fn after_hit_settles_the_acting_owners_take_stage_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(31260211),
                    uid: Some(2),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(
            SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into(),
        )],
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 2).is_none());
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Buff(changes)
            if changes.change.removed.iter().any(|removed| removed.buff.uid == Some(2))
    )));
}

#[test]
fn ally_action_settles_the_acting_owners_take_stage_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30940191),
                        uid: Some(2),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30940191),
                        uid: Some(3),
                        from_uid: Some(20),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(SkillInvocation {
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            ..SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into()
        })],
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 2).is_none());
    assert!(managers.buff.snapshot(20, 3).is_some());
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Buff(changes)
            if changes.origin.key.opcode == 212
                && changes.change.removed.iter().any(|removed| removed.buff.uid == Some(2))
    )));
}
