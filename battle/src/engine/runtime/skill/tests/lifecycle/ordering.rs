use super::*;

#[test]
fn source_opening_and_pre_effect_reactions_precede_action_cost() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                passive_skill: vec![200],
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut opening = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(1, "AddBuff"),
            vec![31270101, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    opening.conditions = vec![ParsedCondition {
        opcode: 201,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(
            crate::engine::skill::condition::none::NoneMode::SkillActionStart,
        ),
        raw_args: Vec::new(),
    }];
    opening.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&opening.conditions);
    let mut passive = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    passive.conditions = vec![ParsedCondition {
        opcode: 34203,
        type_name: "UseSkillEffectTag".to_owned(),
        kind: ParsedConditionKind::ActiveSkillEffectTag(vec![4]),
        raw_args: vec!["4".to_owned()],
    }];
    passive.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&passive.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![opening],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![passive],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    let mut execution = SkillExecution::with_action_cost(
        TargetContext::default(),
        Some(crate::engine::manager::ex_point::ExPointCommand::Spend(
            crate::engine::manager::ex_point::ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(0, "TestPaidAction"),
                },
                source_uid: 10,
                target_uid: 10,
                delta: -5,
                config_effect: 0,
                effect_type: 0,
            },
        )),
    );
    let mut determinism = RoundDeterminism::default();

    let opening = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(matches!(
        opening.ops.first(),
        Some(SkillEmissionOp {
            op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: 10,
                buff_id: 31270101,
                ..
            }))),
            ..
        })
    ));
    assert!(matches!(
        opening.ops.last(),
        Some(SkillEmissionOp {
            op: RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
            ),
            ..
        }) if action.phase == SkillPhase::SourceOpening
    ));
    let immediate = opening.continuation.expect("source opening continuation");
    assert_eq!(immediate.phase, Some(SkillPhase::Immediate));

    let started = emit_ops(
        immediate,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(matches!(
        started.ops.first().map(|emission| &emission.op),
        Some(RuleOp::Publish(BattleEvent::SkillEffectStarted(action)))
            if action.phase == SkillPhase::Immediate
    ));
    assert_eq!(started.ops.len(), 1);

    let immediate = emit_ops(
        started
            .continuation
            .expect("skill resumes after its start event"),
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(immediate.ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::BeginSkillAction {
            lifecycle: crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
            cost: crate::engine::manager::ex_point::ExPointCommand::Spend(change),
        } if action.phase == SkillPhase::Immediate && change.delta == -5
    )));
}

#[test]
fn paid_skill_publishes_immediate_commands_before_action_start_lifecycle() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30006, "LostLife"),
                vec![1, AttrId::CurrentHp as i32, 100],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    let mut execution = SkillExecution::with_action_cost(
        TargetContext::default(),
        Some(crate::engine::manager::ex_point::ExPointCommand::Spend(
            crate::engine::manager::ex_point::ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(0, "TestPaidAction"),
                },
                source_uid: 10,
                target_uid: 10,
                delta: -5,
                config_effect: 0,
                effect_type: 0,
            },
        )),
    );
    let mut determinism = RoundDeterminism::default();

    let emission = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    let command = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    amount: 100,
                    ..
                })))
            )
        })
        .unwrap();
    let lifecycle = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::BeginSkillAction {
                    lifecycle: crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
                    cost: crate::engine::manager::ex_point::ExPointCommand::Spend(change),
                } if action.phase == SkillPhase::Immediate && change.delta == -5
            )
        })
        .unwrap();
    assert!(command < lifecycle);

    let continuation = emission.continuation.expect("Immediate phase continuation");
    let continuation_emission = emit_ops(
        continuation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(continuation_emission.ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
        ) if action.phase == SkillPhase::HitPassives
    )));
}

#[test]
fn exact_active_buff_subscription_keeps_its_required_after_damage_phase() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31340006),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.phase = Some(SkillPhase::AdditionalDamage);

    let emission = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut SkillExecution::new(TargetContext::default()),
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert_eq!(
        emission.continuation.and_then(|next| next.phase),
        Some(SkillPhase::AfterDamage)
    );
}

#[test]
fn additional_damage_activation_consumes_source_count_before_temporary_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31260151),
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
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

    let mut execution = SkillExecution::new(TargetContext::default());
    let emission = emit_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let phase = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
                ) if action.phase == crate::engine::skill::action::SkillPhase::Immediate
            )
        })
        .unwrap();
    let temporary_buff = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    buff_id: 31260171,
                    ..
                })))
            )
        })
        .unwrap();
    let source_consumption = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                    crate::engine::manager::buff::BuffConsume {
                        selector: crate::engine::manager::buff::BuffSelector::Uid(20),
                        amount: 1,
                        ..
                    }
                )))
            )
        })
        .unwrap();

    assert!(phase < source_consumption);
    assert!(source_consumption < temporary_buff);
    assert!(matches!(
        emission.ops[source_consumption].frame_owner,
        Some(crate::engine::runtime::record::FrameOwner::BuffAct {
            buff_uid: 20,
            buff_id: 31260151,
            key: DefinitionKey {
                opcode: 1026,
                type_name: "CreateMaxHpAdditionalDamageAndRemove",
            },
            ..
        })
    ));
    assert_eq!(emission.ops[temporary_buff].frame_owner, None);

    let mut additional = invocation.clone();
    additional.phase = Some(crate::engine::skill::action::SkillPhase::AdditionalDamage);
    let emission = emit_ops(
        additional,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(!emission.ops.iter().any(|emission| matches!(
        emission.op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(_)))
    )));

    let mut after_hit = invocation.clone();
    after_hit.phase = Some(crate::engine::skill::action::SkillPhase::AfterHit);
    let emission = emit_ops(
        after_hit,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let phase = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
                ) if action.phase == crate::engine::skill::action::SkillPhase::AfterHit
            )
        })
        .unwrap();
    let cleanup = emission
        .ops
        .iter()
        .position(|emission| {
            matches!(
                &emission.op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                    selector: BuffRemoveSelector::ExactId(31260171),
                    ..
                })))
            )
        })
        .unwrap();
    assert!(phase < cleanup);
}
