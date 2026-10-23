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

#[test]
fn scheduled_defender_cleanup_policy_keeps_player_cleanup_in_the_death_frame() {
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(1),
        attr: Some(HeroAttribute {
            hp: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card(100)],
            draw_pile: vec![card(200)],
            deck_num: 2,
        }))
        .unwrap();
    managers.card.add_temp_card_for(10, 300, None, 0, 1);
    managers
        .execute_card(CardCommand::SetTeamCards(
            crate::engine::manager::card::CardSetTeamCards {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestTeamCards"),
                },
                cards: vec![card(400)],
            },
        ))
        .unwrap();
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30005, "LostLife"),
                    vec![1, AttrId::CurrentHp as i32, 1000],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30014, "OriginDamage"),
                    vec![0, AttrId::CurrentHp as i32, 0],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
        ],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(10);
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    let result = drain_queue_with_frames_and_cleanup_owner(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        Vec::new(),
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
    .unwrap();

    assert!(managers.card.hand().is_empty());
    assert!(managers.card.draw_pile().is_empty());
    assert!(managers.card.generated().is_empty());
    assert!(managers.card.team_cards().is_empty());
    assert_eq!(managers.card.ai_queue()[0].uid, Some(-1));

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = &steps[0].act_effect;
    let death = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32)
                && effect.target_id == Some(10)
        })
        .unwrap();
    let removal = effects[death + 1].fight_step.as_ref().unwrap();
    assert_eq!(
        effects[death + 1].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32)
    );
    assert_eq!(
        removal
            .act_effect
            .iter()
            .map(|effect| (effect.effect_type, effect.target_id, effect.team_type))
            .collect::<Vec<_>>(),
        vec![(
            Some(sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32),
            Some(10),
            Some(1),
        )]
    );
}

#[test]
fn non_action_death_settles_after_its_reaction_and_card_cleanup() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3002),
                team_type: Some(1),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(100101),
                team_type: Some(2),
                current_hp: Some(1),
                passive_skill: vec![2345],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestDeathSettlement"),
    };
    let result = run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::from_fight(config::configs::get(), &fight),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin,
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            }),
        ))],
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        if step.act_id == Some(2345) {
            order.push("reaction");
        }
        for effect in &step.act_effect {
            match effect.effect_type {
                Some(value)
                    if value
                        == sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32 =>
                {
                    order.push("cleanup");
                }
                Some(value) if value == sonettobuf::effect_type_enum::EffectType::Dead as i32 => {
                    order.push("death");
                }
                _ => {}
            }
            if let Some(child) = effect.fight_step.as_ref() {
                collect(child, order);
            }
        }
    }

    let mut order = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut order);
    }
    assert_eq!(order, ["reaction", "cleanup", "death"]);
    assert!(managers.card.ai_queue().is_empty());
}

#[test]
fn subscriber_group_finishes_before_death_settlement() {
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
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::EventRule, frame_trigger(&trigger));
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: Some(root.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(-1)),
        caster_frame: None,
    };
    let mut reactions = ReactionBatch {
        after_publish: vec![
            queued(RuleOp::BuffFeatureMarker {
                target_uid: -1,
                effect_type: 137,
                effect_num: 1,
                buff_act_id: 701,
            }),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(701, "TestDamage"),
                    },
                    source_uid: 10,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Heal(crate::engine::manager::hp::HpHeal {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(702, "TestRevive"),
                    },
                    source_uid: 10,
                    target_uid: -1,
                    amount: 1,
                    config_effect: 1,
                    kind: crate::engine::manager::hp::HpHealKind::Revive,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(703, "TestDamageAgain"),
                    },
                    source_uid: 11,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
        ],
        after_settlement: vec![queued(RuleOp::BuffFeatureMarker {
            target_uid: -1,
            effect_type: 357,
            effect_num: 2,
            buff_act_id: 702,
        })],
        ..Default::default()
    };
    reactions.assign_subscriber_death_groups(None);
    let mut queue = reactions.into_ordered().into();
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        for effect in &step.act_effect {
            match (effect.buff_act_id, effect.effect_type) {
                (Some(701), _) => order.push("first"),
                (Some(702), _) => order.push("second"),
                (_, Some(value))
                    if value
                        == sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32 =>
                {
                    order.push("cleanup");
                }
                (_, Some(value))
                    if value == sonettobuf::effect_type_enum::EffectType::Dead as i32 =>
                {
                    order.push("death");
                }
                _ => {}
            }
            if let Some(child) = effect.fight_step.as_ref() {
                collect(child, order);
            }
        }
    }

    let mut order = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut order);
    }
    assert_eq!(order, ["first", "cleanup", "second", "death"]);
    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                crate::engine::runtime::executor::RuleOutcome::DeathSettlement(death) => {
                    Some(*death)
                }
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
fn subscriber_group_does_not_settle_a_revived_target() {
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
    let origin = |opcode, name| CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(opcode, name),
    };
    let mut reactions = ReactionBatch {
        after_publish: vec![
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: origin(701, "TestDamage"),
                    source_uid: 10,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Heal(crate::engine::manager::hp::HpHeal {
                    origin: origin(702, "TestRevive"),
                    source_uid: 10,
                    target_uid: -1,
                    amount: 1,
                    config_effect: 1,
                    kind: crate::engine::manager::hp::HpHealKind::Revive,
                }),
            ))),
        ],
        ..Default::default()
    };
    reactions.assign_subscriber_death_groups(None);
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

    assert_eq!(managers.hp.current(-1), 1);
    assert!(!result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::DeathSettlement(_)
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

#[test]
fn queued_reactions_preserve_dispatch_pass_order() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let reaction = |owner_uid, skill_id, key| {
        (
            SkillSubscriber {
                owner_uid,
                skill_id,
                slot_index: Some(0),
                key: SubscriptionKey::new(EventKind::SkillEffectStarted, key),
            },
            RuleOp::Skill(
                SkillRequest {
                    source_uid: owner_uid,
                    skill_id,
                }
                .into(),
            ),
        )
    };
    let started = BattleEvent::SkillEffectStarted(
        crate::engine::skill::action::SkillActionEvent {
            source_uid: 10,
            skill_id: 31050151,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: Vec::new(),
            phase: crate::engine::skill::action::SkillPhase::Immediate,
            skill_slot: 0,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 1,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        }
        .into(),
    );
    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![reaction(
                -1,
                109380003,
                DefinitionKey::new(502202, "ActiveUseSkill"),
            )],
            following: vec![dispatcher::DispatchBatch {
                skills: vec![reaction(
                    10,
                    433911,
                    DefinitionKey::new(403203, "SkillExtraType"),
                )],
                ..Default::default()
            }],
            ..Default::default()
        },
        &started,
        Some(&[0]),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(
        queued
            .iter()
            .map(|reaction| reaction
                .subscriber
                .as_ref()
                .map(|subscriber| subscriber.owner_uid))
            .collect::<Vec<_>>(),
        vec![Some(-1), Some(10)]
    );
}
