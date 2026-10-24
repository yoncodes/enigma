use super::*;

#[test]
fn recover_power_generated_action_settles_consumed_hand_once() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    skill_group1: vec![100, 101],
                    passive_skill: vec![31050143, 433911],
                    power_infos: vec![PowerInfo {
                        power_id: Some(EUREKA_RESOURCE_ID),
                        num: Some(0),
                        max: Some(5),
                    }],
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31050146),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ex_point: Some(0),
                    ex_point_max: Some(5),
                    skill_group1: vec![200, 201],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                passive_skill: vec![109380003],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(20),
                    skill_id: Some(200),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(100),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(20),
                    skill_id: Some(200),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31050143, 31050151, 433911, 109380003],
        [],
    );
    catalog.insert(ParsedSkillEffect {
        skill_id: 500,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(
                60125,
                "RecoverPowerAndDelCardsUseSkill",
                vec![31050151, 210],
            ),
            TargetRequest::self_only(),
        )],
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(SkillInvocation {
            mode: SkillExecutionMode::Active,
            ..SkillRequest {
                source_uid: 10,
                skill_id: 500,
            }
            .into()
        })],
    )
    .unwrap();

    assert_eq!(managers.card.hand().len(), 1);
    assert_eq!(managers.card.hand()[0].skill_id, Some(201));
    assert_eq!(managers.ex_point.get(20), 1);
    assert_eq!(
        result
            .outcomes
            .iter()
            .filter(|outcome| matches!(outcome, RuleOutcome::ActionHandSettlement(_)))
            .count(),
        1
    );
    let settlement = result
        .outcomes
        .iter()
        .position(|outcome| matches!(outcome, RuleOutcome::ActionHandSettlement(_)))
        .unwrap();
    let completion = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
                ) if action.skill_id == 31050151
            )
        })
        .unwrap();
    assert!(settlement < completion);
    fn generated_skill_frame(frame: &SemanticFrame) -> Option<&SemanticFrame> {
        if matches!(
            frame.owner,
            FrameOwner::Skill {
                skill_id: 31050151,
                ..
            }
        ) {
            return Some(frame);
        }
        frame.items.iter().find_map(|item| match item {
            FrameItem::Child(child) => generated_skill_frame(child),
            _ => None,
        })
    }
    let frame = result
        .frames
        .iter()
        .find_map(generated_skill_frame)
        .expect("generated action owns a skill frame");
    let reaction = frame
        .items
        .iter()
        .find_map(|item| match item {
            FrameItem::Child(child)
                if matches!(
                    child.owner,
                    FrameOwner::Skill {
                        skill_id: 433911,
                        ..
                    }
                ) =>
            {
                Some(child)
            }
            _ => None,
        })
        .expect("the actor start reaction owns its frame");
    let ex_point = reaction
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::ExPoint(_))))
        .expect("settlement grants the consumed ally card's owner Moxie");
    let compose = reaction
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Card(changes) if changes.kind == CardChangeKind::Composed)))
        .expect("settlement composes the remaining hand");
    let reaction_buff = reaction
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Buff(_))))
        .expect("the actor reaction applies its buff after hand settlement");
    let force_field = frame
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Child(child) if matches!(child.owner, FrameOwner::BuffAct { buff_id: 31050146, .. })))
        .expect("generated action pays its force-field cost");
    let damage = frame
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Hp(_))))
        .expect("generated action deals damage");
    assert!(ex_point < compose);
    assert!(compose < reaction_buff);
    let reaction = frame
        .items
        .iter()
        .position(|item| matches!(item, FrameItem::Child(child) if matches!(child.owner, FrameOwner::Skill { skill_id: 433911, .. })))
        .unwrap();
    assert!(reaction < force_field);
    assert!(force_field < damage);
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

    let event = BattleEvent::Kind(crate::engine::event::kind::EventKind::SkillAction);
    let queued = dispatch_owner_reactions(
        &pool,
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &event,
        &[10],
    )
    .unwrap();
    assert!(
        queued
            .iter()
            .filter_map(|queued| queued.subscriber.as_ref())
            .all(|subscriber| subscriber.owner_uid == 10 && subscriber.group.is_some())
    );
    assert!(matches!(
        queued.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));

    let result = run_owner_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
        &[10],
    )
    .unwrap();

    assert_eq!(result.outcomes.len(), 1);
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(-1), 0);
}

#[test]
fn skill_start_phase_merge_reassigns_one_subscriber_completion() {
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
    let action_path = [0];
    let mut before = ReactionBatch {
        before_publish: vec![queued(701)],
        ..Default::default()
    };
    let mut after = ReactionBatch {
        after_publish: vec![queued(702)],
        ..Default::default()
    };
    before.assign_subscriber_death_groups(Some(&action_path));
    after.assign_subscriber_death_groups(Some(&action_path));
    before.before_publish.append(&mut after.after_publish);
    before.assign_subscriber_death_groups(Some(&action_path));
    let queued = before.into_ordered();

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
fn deferred_hit_consumes_the_attack_buff_before_after_hit_completes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    attack: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(430111),
                    from_uid: Some(10),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30015, "OriginDamageCanCrit"),
                vec![0, AttrId::CurrentHp as i32, 1_000],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
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
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 30).is_none());
    let removal = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Buff(changes)
                    if changes.change.removed.iter().any(|removed| removed.buff.uid == Some(30))
            )
        })
        .unwrap();
    let after_hit = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
                ) if action.phase == crate::engine::skill::action::SkillPhase::AfterHit
            )
        })
        .unwrap();
    assert!(removal < after_hit);
}

#[test]
fn non_attack_device_after_hit_behaviors_run_before_take_stage_buff_settles() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point_type: Some(4),
                ex_point: Some(0),
                ex_point_max: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(103374322),
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
    let mut after_hit = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60292, "AddDeviceExPoint"),
            vec![7],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    after_hit.conditions = vec![ParsedCondition {
        opcode: 210,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillActionAfterHit),
        raw_args: Vec::new(),
    }];
    after_hit.compiled_route = ConditionRoute::compile(&after_hit.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 31490111,
        slots: vec![after_hit],
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(SkillInvocation {
            mode: crate::engine::skill::action::SkillExecutionMode::Device,
            ..SkillRequest {
                source_uid: 10,
                skill_id: 31490111,
            }
            .into()
        })],
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 2).is_none());
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Buff(changes)
            if changes.change.removed.iter().any(|removed| removed.buff.uid == Some(2))
    )));
    let removed = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Buff(changes)
                    if changes.change.removed.iter().any(|removed| removed.buff.uid == Some(2))
            )
        })
        .unwrap();
    let resource = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                    change,
                    ..
                }) if change.applied_delta == 7
            )
        })
        .unwrap();
    assert!(resource < removed);
}
