use super::*;

#[test]
fn team_injury_count_is_resynchronized_between_skill_phases() {
    let mut execution = SkillExecution::new(TargetContext::default());

    execution.sync_team_injury_count(13);
    execution.sync_team_injury_count(14);

    assert_eq!(execution.team_injury_count_round, 14);
    assert_eq!(execution.context.team_injury_count_round, 14);
}

#[test]
fn config_extra_actions_publish_as_actions_without_changing_other_nested_skills() {
    use crate::engine::skill::{action::SkillExecutionMode, condition::extra::ExtraSkillKind};

    assert_eq!(
        action_mode(
            SkillExecutionMode::Nested,
            Some(ExtraSkillKind::ExtraAction)
        ),
        SkillExecutionMode::Active
    );
    assert_eq!(
        action_mode(SkillExecutionMode::Nested, Some(ExtraSkillKind::Riposte)),
        SkillExecutionMode::Nested
    );
    assert_eq!(
        action_mode(SkillExecutionMode::Nested, None),
        SkillExecutionMode::Nested
    );
}

#[test]
fn entity_ultimate_kind_overrides_the_catalog_for_only_that_ultimate() {
    use crate::engine::{
        manager::entity::EntitySkillCommand,
        skill::{
            action::SkillExecutionMode,
            condition::extra::ExtraSkillKind,
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
        },
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ex_skill: Some(900),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_entity_skill(EntitySkillCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(100012, "EzioBigSkillWeapon2"),
            },
            target_uid: 10,
            ultimate_kind: ExtraSkillKind::ExtraAction,
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 900,
        slots: Vec::new(),
    });
    let mut execution = SkillExecution::new(TargetContext::default());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 900,
    }
    .into();
    let mut determinism = RoundDeterminism::default();
    let mut ops = Vec::new();
    loop {
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
        ops.extend(emission.ops);
        let Some(continuation) = emission.continuation else {
            break;
        };
        invocation = continuation;
    }

    assert!(ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::SkillLifecycle(crate::engine::skill::action::SkillLifecycle::ActionCompleted(
            action
        )) if action.skill_id == 900
            && action.extra_skill_kind == ExtraSkillKind::ExtraAction.id()
    )));
    assert_eq!(
        action_mode(
            SkillExecutionMode::Nested,
            Some(ExtraSkillKind::ExtraAction)
        ),
        SkillExecutionMode::Active
    );
}

#[test]
fn zero_cost_skill_publishes_exact_pre_effect_event_before_its_commands() {
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
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![passive],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
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

    assert!(matches!(
        emission.ops.first().map(|emission| &emission.op),
        Some(RuleOp::Publish(BattleEvent::SkillEffectStarted(action)))
            if action.phase == crate::engine::skill::action::SkillPhase::Immediate
    ));
    assert!(matches!(
        emission.ops.get(1).map(|emission| &emission.op),
        Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss { amount: 100, .. }
        ))))
    ));
    assert!(matches!(
        emission.ops.get(2).map(|emission| &emission.op),
        Some(RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
        )) if action.phase == crate::engine::skill::action::SkillPhase::Immediate
    ));
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

#[test]
fn repeated_direct_use_destinations_are_alternatives_within_one_parent_skill() {
    let behavior = ParsedBehavior::new(50008, "DirectUseSkill", vec![20]);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 20,
    }
    .into();
    let outputs = vec![SkillEmissionOp {
        op: RuleOp::Skill(invocation),
        owner: crate::engine::skill::behavior::registry::OutputOwner::Skill,
        consequence: ConsequencePolicy::Default,
        frame_owner: None,
    }];

    let definition = behavior::registry::find(&behavior).unwrap();
    assert!(skill_destination_already_emitted(
        &outputs, definition, &behavior
    ));
    let other = ParsedBehavior::new(50008, "DirectUseSkill", vec![21]);
    assert!(!skill_destination_already_emitted(
        &outputs,
        behavior::registry::find(&other).unwrap(),
        &other,
    ));
    let repeated = ParsedBehavior::new(50012, "DirectUseSkillNoAct", vec![20]);
    assert!(!skill_destination_already_emitted(
        &outputs,
        behavior::registry::find(&repeated).unwrap(),
        &repeated,
    ));
}

#[test]
fn unsupported_behavior_is_skipped_without_aborting_the_skill() {
    crate::test_support::init_config();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert_issue(
        100,
        RuleIssue {
            effect_id: 100,
            slot: 1,
            opcode: Some(999),
            type_name: Some("Missing".to_owned()),
            raw: "999".to_owned(),
            reason: RuleIssueReason::UnsupportedBehavior,
        },
    );
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let ops = emit_all_ops(
        SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into(),
        &BattleManagers::seeded(&fight),
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn configured_after_damage_buff_targets_every_resolved_action_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(10_000),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(10_000),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: 30810481,
            source_uid: 10,
            target_code: 201,
            targets: vec![-1, -2],
            additional_targets: Vec::new(),
            crit_targets: Vec::new(),
            additional_crit_targets: Vec::new(),
        },
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30810481,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-1);
    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert_eq!(managers.buff.max_id_or_type_layer(-1, 4150001), 2);
    assert_eq!(managers.buff.max_id_or_type_layer(-2, 4150001), 2);
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        ) if action.target_uids == [-1, -2]
    )));
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
        ) if action.phase == crate::engine::skill::action::SkillPhase::Immediate
            && action.target_uids == [-1, -2]
    )));
}

#[test]
fn aggregate_buff_count_with_target_999_applies_to_the_action_targets() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        team_type: Some(if uid > 0 { 1 } else { 2 }),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11), entity(12), entity(13)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1),
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(530000712),
                        from_uid: Some(-2),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..entity(-2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    let seeded = BattleManagers::seeded(&fight);
    let effect = crate::engine::skill::effect::catalog::global()
        .get(530000531)
        .unwrap();
    assert!(
        crate::engine::skill::rule::ownership::behavior_is_owned_by_buff_act(
            &effect.slots[0],
            -2,
            &seeded,
        ),
        "hp={} buff={:?} features={:?} slot={:?}",
        seeded.hp.current(-2),
        seeded.buff.snapshot(-2, 20),
        seeded.buff.active_features(&seeded.hp),
        effect.slots[0]
    );
    let ops = emit_all_ops(
        SkillRequest {
            source_uid: -2,
            skill_id: 530000531,
        }
        .into(),
        &seeded,
        &TargetPool::from_fight(&fight),
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let targets = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if grant.buff_id == 530000412 =>
            {
                Some(grant.target_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(targets.is_empty());

    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -2,
        skill_id: 530000531,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &TargetPool::from_fight(&fight),
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let present = [10, 11, 12, 13].map(|uid| (uid, managers.buff.has_buff_id(uid, 530000412)));
    assert!(
        present.iter().all(|(_, present)| *present),
        "present={present:?} events={:?}",
        result.events
    );
}
