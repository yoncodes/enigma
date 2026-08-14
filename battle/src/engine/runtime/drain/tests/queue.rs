use super::*;

#[test]
fn active_hit_deferral_carries_only_its_primary_skill_hp_loss() {
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(1, "SkillDamage"),
    };
    let hp_loss = |skill_id, target_uid| BattleEvent::HpLost {
        origin,
        source_uid: 10,
        skill_id,
        target_uid,
        amount: 100,
        buff_uid: None,
    };
    let hit = |skill_id, target_uid, damage_from| {
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin,
            source_uid: 10,
            target_uid,
            skill_id,
            amount: 100,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from,
            assassinate: false,
            ignore_riposte: false,
        })
    };
    let primary_loss = hp_loss(1, -1);
    let primary_hit = hit(1, -1, crate::engine::manager::hp::HurtDamageFromType::Skill);
    let death = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
        source_uid: 10,
        target_uid: -1,
    });
    let unrelated_loss = hp_loss(2, -2);
    let effect_loss = hp_loss(3, -3);
    let effect_hit = hit(
        3,
        -3,
        crate::engine::manager::hp::HurtDamageFromType::SkillEffect,
    );
    let events = vec![
        primary_loss.clone(),
        primary_hit.clone(),
        death.clone(),
        unrelated_loss.clone(),
        effect_loss.clone(),
        effect_hit.clone(),
    ];

    let (immediate, deferred) = split_active_hit_events(
        events,
        vec![
            vec![primary_loss.clone(), primary_hit.clone(), death.clone()],
            vec![unrelated_loss.clone()],
            vec![effect_loss.clone(), effect_hit.clone()],
        ],
    );

    assert_eq!(immediate, vec![death, unrelated_loss, effect_loss]);
    assert_eq!(deferred, vec![primary_loss, primary_hit, effect_hit]);
}

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
            subscriber_owner_uid: None,
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
fn active_skill_rates_freeze_after_immediate_reactions_and_before_later_gauge_changes() {
    use crate::engine::{
        manager::gauge::{GaugeCommand, GaugeOperation},
        mechanic::lingering_glow,
        skill::{
            action::{SkillModifiers, SkillPhase, SkillRateAmount, SkillRateModifier},
            rule::output::BattleCommand,
        },
    };

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                passive_skill: vec![200],
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60243, "CrystalAddSkillRate"),
    };
    let gauge_key = lingering_glow::key(1);
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::Enable { max: Some(1_000) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::ChangeValue { delta: 106 },
        ))
        .unwrap();

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    let mut reaction = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60191, "BloodPoolValueChange"),
            vec![33_000, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    reaction.conditions = vec![ParsedCondition {
        opcode: 203,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillActionStart),
        raw_args: Vec::new(),
    }];
    reaction.compiled_route = ConditionRoute::compile(&reaction.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![reaction],
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
    let mut continuation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    continuation.mode = SkillExecutionMode::Active;
    continuation.phase = Some(SkillPhase::Damage);
    continuation.target = SkillTarget::Explicit(-1);
    let execution = SkillExecution::with_modifiers(
        TargetContext::default(),
        SkillModifiers {
            rates: vec![
                SkillRateModifier::new(
                    -1,
                    60243,
                    SkillRateAmount::gauge_current(gauge_key, 1_000, 4, 1),
                    true,
                ),
                SkillRateModifier::new(
                    -1,
                    60243,
                    SkillRateAmount::gauge_current(gauge_key, 1_000, 4, 1),
                    true,
                ),
            ],
            ..Default::default()
        },
    );

    let mut frames = Vec::new();
    let frame_path = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            card_index: 0,
            target_uid: Some(-1),
        },
        FrameTrigger::Active,
    );
    let queued = |op, skill_execution| QueuedOp {
        op,
        trigger: SkillOpTrigger::Active,
        skill_execution,
        frame_path: Some(frame_path.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber_owner_uid: None,
    };
    let mut queue = VecDeque::from([
        queued(
            RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::PhaseCompleted(
                    crate::engine::skill::action::SkillActionEvent {
                        source_uid: 10,
                        skill_id: 100,
                        target_uid: -1,
                        target_uids: vec![-1],
                        attacked_target_uids: Vec::new(),
                        phase: SkillPhase::Immediate,
                        skill_slot: 1,
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
                        extra_skill_kind: 0,
                        mode: SkillExecutionMode::Active,
                        teammate_injury_count: 0,
                        teammate_injury_count_not_reset: 0,
                        team_injury_count_round: 0,
                        card_enchants: Vec::new(),
                        buff_additions: Vec::new(),
                    },
                ),
            ),
            None,
        ),
        queued(RuleOp::FreezeActiveSkillRates, None),
        queued(
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
                origin,
                gauge_key,
                GaugeOperation::ChangeValue { delta: 10 },
            ))),
            None,
        ),
        queued(RuleOp::Skill(continuation), Some(execution)),
    ]);

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

    assert_eq!(managers.gauge.get(gauge_key).unwrap().current, 149);
    assert_eq!(
        result
            .outcomes
            .iter()
            .map(RuleOutcome::applied_damage)
            .sum::<i32>(),
        2_112
    );
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
