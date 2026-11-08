use super::*;

#[test]
fn active_invocation_initializes_the_condition_mode() {
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
    invocation.mode = SkillExecutionMode::Active;
    let mut execution = SkillExecution::new(TargetContext::default());

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert_eq!(
        execution.context.active_skill_mode,
        SkillExecutionMode::Active
    );
}

#[test]
fn unsupported_behavior_slot_fails_before_runtime_emission() {
    crate::test_support::init_config();
    let configured = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut unsupported = configured
        .get(311401351)
        .unwrap()
        .slots
        .iter()
        .find(|slot| {
            slot.behavior.spec.kind
                == crate::engine::skill::behavior::classify::BehaviorKind::ConsumeExPointAddAttr
        })
        .unwrap()
        .clone();
    unsupported.target.code = 202;
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
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
        slots: vec![unsupported],
    });
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();

    assert!(matches!(
        emit_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            &mut SkillExecution::new(TargetContext::default()),
            &SkillOpTrigger::Active,
        ),
        Err(SkillOpError::MissingBehaviorOp {
            skill_id,
            key,
        }) if skill_id == 100 && key == DefinitionKey::new(60174, "ConsumeExPointAddAttr")
    ));
}

#[test]
fn completed_action_combines_base_and_additional_moxie() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3114),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(9),
                    ex_point_max: Some(13),
                    ex_skill: Some(311401351),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        defense: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3147),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(0),
                    ex_point_max: Some(5),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(116371003),
                team_type: Some(2),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    defense: Some(100),
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
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 311401351,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.card_index = 5;
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::with_payment(
        TargetContext::default(),
        Some(crate::engine::skill::action::ActionPayment {
            cost: crate::engine::manager::ex_point::ExPointCommand::Spend(
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
            ),
            composition: Vec::new(),
        }),
    );
    let mut determinism = RoundDeterminism::default();
    let mut completed = None;
    let mut immediate_additional_moxie = None;

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
        completed = emission
            .ops
            .iter()
            .find_map(|emission| match &emission.op {
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(action),
                ) => Some(action.clone()),
                _ => None,
            })
            .or(completed);
        immediate_additional_moxie = emission
            .ops
            .iter()
            .find_map(|emission| match &emission.op {
                RuleOp::BeginSkillAction {
                    lifecycle: crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
                    ..
                }
                | RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
                ) if action.phase == SkillPhase::Immediate => Some(action.additional_moxie),
                _ => None,
            })
            .or(immediate_additional_moxie);
        let Some(continuation) = emission.continuation else {
            break;
        };
        invocation = continuation;
    }

    let completed = completed.expect("action must complete");
    assert_eq!(immediate_additional_moxie, Some(4));
    assert_eq!(completed.additional_moxie, 4);
    assert_eq!(completed.moxie_spent, 9);
}

#[test]
fn direct_big_completed_action_uses_the_gross_spend() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
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
    invocation.mode = SkillExecutionMode::DirectBig;
    invocation.moxie_spent = 5;
    let mut execution = SkillExecution::new(TargetContext::default());
    let mut determinism = RoundDeterminism::default();
    let mut completed = None;

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
        completed = emission
            .ops
            .iter()
            .find_map(|emission| match &emission.op {
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(action),
                ) => Some(action.clone()),
                _ => None,
            })
            .or(completed);
        let Some(continuation) = emission.continuation else {
            break;
        };
        invocation = continuation;
    }

    let completed = completed.expect("action must complete");
    assert_eq!(completed.additional_moxie, 0);
    assert_eq!(completed.moxie_spent, 5);
}

#[test]
fn direct_nested_invocation_owns_its_attack_context() {
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(100, 1000);
    catalog.insert_effect_tag(100, 11);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Direct;
    let mut execution = SkillExecution::new(TargetContext::default());

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert_eq!(
        execution.context.active_skill_mode,
        SkillExecutionMode::Direct
    );
    assert!(execution.context.active_skill_is_attack);
    assert_eq!(execution.context.active_skill_effect_tag, 11);
    assert!(execution.context.direct_skill_body);
    assert!(SkillExecutionMode::Direct.completes_action());
    assert!(!crate::engine::skill::action::performs_extra_action(
        0,
        SkillExecutionMode::Direct,
        0
    ));
}

#[test]
fn synchronization_uses_the_extra_action_lane_and_completes_its_own_action() {
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
    invocation.mode = SkillExecutionMode::Synchronization;
    let mut execution = SkillExecution::new(TargetContext::default());

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

    assert_eq!(
        execution.context.active_skill_mode,
        SkillExecutionMode::Synchronization
    );
    assert!(crate::engine::skill::action::performs_extra_action(
        0,
        execution.context.active_skill_mode,
        0
    ));
    assert!(ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        )
            if action.mode == SkillExecutionMode::Synchronization
    )));
}

#[test]
fn team_injury_count_is_resynchronized_between_skill_phases() {
    let mut execution = SkillExecution::new(TargetContext::default());

    execution.sync_team_injury_count(13);
    execution.sync_team_injury_count(14);

    assert_eq!(execution.team_injury_count_round, 14);
    assert_eq!(execution.context.team_injury_count_round, 14);
}

#[test]
fn config_extra_actions_and_ripostes_publish_as_actions_without_changing_other_nested_skills() {
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
        SkillExecutionMode::Active
    );
    assert_eq!(
        action_mode(SkillExecutionMode::Nested, Some(ExtraSkillKind::Reinforced)),
        SkillExecutionMode::Nested
    );
    assert_eq!(
        action_mode(SkillExecutionMode::Nested, None),
        SkillExecutionMode::Nested
    );
    assert_eq!(
        action_mode(
            SkillExecutionMode::NestedExtra,
            Some(ExtraSkillKind::ExtraAction)
        ),
        SkillExecutionMode::NestedExtra
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

    let mut determinism = RoundDeterminism::default();
    let emission = emit_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(matches!(
        emission.ops.first().map(|emission| &emission.op),
        Some(RuleOp::Publish(BattleEvent::SkillEffectStarted(action)))
            if action.phase == crate::engine::skill::action::SkillPhase::Immediate
    ));
    assert_eq!(emission.ops.len(), 1);
    let emission = emit_ops(
        emission
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
    assert!(matches!(
        emission.ops.first().map(|emission| &emission.op),
        Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss { amount: 100, .. }
        ))))
    ));
    assert!(matches!(
        emission.ops.get(1).map(|emission| &emission.op),
        Some(RuleOp::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
        )) if action.phase == crate::engine::skill::action::SkillPhase::Immediate
    ));
}
