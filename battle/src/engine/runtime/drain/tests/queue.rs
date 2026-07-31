use super::*;

#[test]
fn after_skill_reaction_waits_for_remaining_ops_in_the_skill_frame() {
    fn queued(
        skill_id: i32,
        frame_path: Option<FramePath>,
        parent_path: Option<FramePath>,
    ) -> QueuedOp {
        QueuedOp {
            op: RuleOp::Skill(
                SkillRequest {
                    source_uid: 10,
                    skill_id,
                }
                .into(),
            ),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path,
            parent_path,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
        }
    }

    let skill_frame = vec![0];
    let frame_group = Rc::new(RefCell::new(Some(skill_frame.clone())));
    let mut grouped = queued(5, None, None);
    grouped.frame_group = Some(frame_group);
    let mut queue = VecDeque::from([
        queued(1, Some(skill_frame.clone()), None),
        queued(2, None, Some(skill_frame.clone())),
        grouped,
        queued(4, Some(vec![1]), None),
    ]);

    insert_after_frame(
        &mut queue,
        &skill_frame,
        [queued(3, None, Some(skill_frame.clone()))],
    );

    let skill_ids = queue
        .into_iter()
        .map(|queued| match queued.op {
            RuleOp::Skill(invocation) => invocation.plan.skill_id,
            _ => unreachable!("test queue contains only skill invocations"),
        })
        .collect::<Vec<_>>();
    assert_eq!(skill_ids, vec![1, 2, 5, 3, 4]);
}

#[test]
fn after_current_action_skill_starts_after_parent_action_completed() {
    crate::test_support::init_config();
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    assert!(managers.emanation.select(10, 300));

    let parent_skill = 31340151;
    let child_skill = 31340152;
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: parent_skill,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60242, "CrystalReuse"),
                vec![1_000, child_skill, 1],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: child_skill,
        slots: Vec::new(),
    });
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([child_skill]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: parent_skill,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let mut completed_skills = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::SkillAction(action) => Some(action.skill_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    completed_skills.dedup();
    assert_eq!(completed_skills, vec![parent_skill, child_skill]);
}

#[test]
fn manager_followup_runs_the_skill_emitted_after_shell_progress() {
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Shell(
            ShellCommand::AccumulateAndUseSkill {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60135, "ShellUseSkill"),
                },
                source_uid: 10,
                target_uid: -1,
                threshold: 5,
                delta: 5,
                skill_id: 200,
            },
        ))],
    )
    .unwrap();

    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::SkillAction(action)
            if action.source_uid == 10 && action.skill_id == 200 && action.target_uid == -1
    )));
}

#[test]
fn dead_entity_cannot_execute_an_already_queued_active_skill() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 200,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(result.events.is_empty());
    assert!(result.frames.is_empty());
}

#[test]
fn attack_followup_does_not_start_without_a_living_configured_target() {
    crate::test_support::init_config();
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
            sub_entitys: vec![FightEntityInfo {
                uid: Some(-20),
                current_hp: Some(100),
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
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(200, 1000);
    catalog.insert_logic_target(200, 202);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 200,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(result.events.is_empty());
    assert!(result.frames.is_empty());
}
