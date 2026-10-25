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
