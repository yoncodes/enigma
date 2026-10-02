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
            caster_frame: None,
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
        caster_frame: None,
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

#[test]
fn lethal_injury_inflicted_by_an_attack_waits_for_the_next_attack() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312431212], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312431212,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        crate::engine::skill::action::SkillModifiers::default(),
    )
    .unwrap();

    // "Inflicts 2 stacks of [Lethal Injury] on the target hit": the hit itself is not an Assassination.
    assert_eq!(managers.buff.max_id_or_type_layer(-1, 31240121), 2);
}

#[test]
fn lethal_injury_consumption_is_its_appliers_buff_act_step() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(31240121),
                    from_uid: Some(10),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31090111], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 11,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let result = run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        crate::engine::skill::action::SkillModifiers::default(),
    )
    .unwrap();

    fn lethal_injury_steps(effects: &[sonettobuf::ActEffect], found: &mut Vec<Option<i64>>) {
        for step in effects
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            if step.act_id == Some(31240121) {
                found.push(step.from_id);
            }
            lethal_injury_steps(&step.act_effect, found);
        }
    }
    let mut found = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        lethal_injury_steps(&step.act_effect, &mut found);
    }
    assert_eq!(managers.buff.max_id_or_type_layer(-1, 31240121), 0);
    assert_eq!(found, vec![Some(10)]);
}

#[test]
fn each_gash_type_on_the_main_target_casts_sparta_kick_again() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sparta_kicks = |target_buffs: Vec<BuffInfo>| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    buffs: target_buffs,
                    ..entity(-1)
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451115], []);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 312451115,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
        let result = run_action(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [],
            invocation,
        )
        .unwrap();
        let kicks = result
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(
                    event,
                    crate::engine::event::payload::BattleEvent::SkillAction(action)
                        if action.skill_id == 312451011
                            && action.phase == crate::engine::skill::action::SkillPhase::Immediate
                )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        // The recast follows the mass attack's completion and its ally-action reactions.
        let completed = result.events.iter().position(|event| {
            matches!(
                event,
                crate::engine::event::payload::BattleEvent::AllyAction(action)
                    if action.skill_id == 312451115
            )
        });
        assert!(
            kicks
                .iter()
                .all(|kick| completed.is_some_and(|completed| completed < *kick))
        );
        kicks.len()
    };
    let kick_gash = BuffInfo {
        uid: Some(30),
        buff_id: Some(312451011),
        from_uid: Some(10),
        duration: Some(3),
        ..Default::default()
    };

    let arrow_gash = BuffInfo {
        uid: Some(31),
        buff_id: Some(312451021),
        ..kick_gash.clone()
    };

    assert_eq!(sparta_kicks(Vec::new()), 0);
    assert_eq!(sparta_kicks(vec![kick_gash.clone()]), 1);
    assert_eq!(sparta_kicks(vec![kick_gash, arrow_gash]), 2);
}

#[test]
fn shell_necklace_cast_follows_the_attack_inside_its_own_step() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    passive_skill: vec![31090144],
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31090111),
                        from_uid: Some(10),
                        layer: Some(15),
                        ..Default::default()
                    }],
                    ..entity(10)
                },
                // Flutterpage gains Gust after any ally takes an action.
                FightEntityInfo {
                    passive_skill: vec![31050141],
                    ..entity(11)
                },
            ],
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
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 31090144, 31050141], []);
    // Six deployments or retrievals so far; the attack's deployment is the seventh.
    crate::engine::mechanic::shell::execute(
        &mut managers,
        ShellCommand::AccumulateAndUseSkill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60135, "ShellUseSkill"),
            },
            source_uid: 10,
            target_uid: -1,
            threshold: 7,
            delta: 6,
            skill_id: 31090114,
        },
    )
    .unwrap();
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;

    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();

    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(31090111))
        .expect("the attack projects a step");
    let children = attack
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .collect::<Vec<_>>();
    let ally_reaction = children
        .iter()
        .position(|step| step.act_id == Some(31050141))
        .expect("the ally-action reaction reacts to the attack");
    let last_child = children.last().expect("the attack has reaction steps");
    assert!(ally_reaction < children.len() - 1);
    assert_eq!(last_child.act_id, Some(31090144));
    assert!(
        last_child
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|step| step.act_id == Some(31090114))
    );
}

fn direct_use_passive(
    skill_id: i32,
    condition: i32,
    condition_target: i32,
    cast_skill_id: i32,
) -> ParsedSkillEffect {
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(50008, "DirectUseSkill"),
            vec![cast_skill_id],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: condition,
        type_name: "None".to_owned(),
        kind: crate::engine::skill::condition::registry::parse(condition, "None", &[])
            .expect("registered condition"),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    slot.condition_target = TargetRequest {
        code: condition_target,
        raw: Vec::new(),
    };
    slot.limit = 1;
    ParsedSkillEffect {
        skill_id,
        slots: vec![slot],
    }
}

fn attack_with_passives(
    attacker_passives: Vec<i32>,
    ally_passives: Vec<i32>,
    extra: Vec<ParsedSkillEffect>,
    extra_kind: Option<crate::engine::skill::condition::extra::ExtraSkillKind>,
) -> DrainResult {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    passive_skill: attacker_passives,
                    ..entity(10)
                },
                FightEntityInfo {
                    passive_skill: ally_passives,
                    ..entity(11)
                },
            ],
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
    let mut catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 31050141], []);
    for effect in extra {
        catalog.insert(effect);
    }
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    // A cast follow-up stays nested until dispatch settles it as an action.
    match extra_kind {
        Some(kind) => invocation.extra_skill_kind = Some(kind),
        None => invocation.mode = SkillExecutionMode::Active,
    }
    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap()
}

fn assert_passive_cast_follows_the_attack(
    condition: i32,
    extra_kind: Option<crate::engine::skill::condition::extra::ExtraSkillKind>,
) {
    let result = attack_with_passives(
        vec![400],
        vec![31050141],
        vec![
            direct_use_passive(400, condition, 103, 401),
            ParsedSkillEffect {
                skill_id: 401,
                slots: Vec::new(),
            },
        ],
        extra_kind,
    );

    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(31090111))
        .expect("the attack projects a step");
    let children = attack
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .collect::<Vec<_>>();
    let ally_reaction = children
        .iter()
        .position(|step| step.act_id == Some(31050141))
        .expect("the ally-action reaction reacts to the attack");
    let passive = children.last().expect("the attack has reaction steps");
    let passive_children = passive
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter_map(|step| step.act_id)
        .collect::<Vec<_>>();
    assert!(ally_reaction < children.len() - 1);
    assert_eq!(passive.act_id, Some(400));
    assert_eq!(passive_children, vec![401]);
}

#[test]
fn a_passive_cast_during_an_action_follows_it_inside_one_passive_step() {
    assert_passive_cast_follows_the_attack(402, None);
}

#[test]
fn a_passive_cast_as_a_cast_follow_up_attack_starts_follows_it() {
    assert_passive_cast_follows_the_attack(
        201,
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp),
    );
}

#[test]
fn a_cast_from_a_completed_action_reaction_runs_at_once() {
    // Like Flutterpage's "after any ally takes an action" (212 on all allies).
    let result = attack_with_passives(
        Vec::new(),
        vec![300],
        vec![
            direct_use_passive(300, 212, 101, 301),
            ParsedSkillEffect {
                skill_id: 301,
                slots: Vec::new(),
            },
        ],
        None,
    );

    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::SkillAction(action) if action.skill_id == 301
    )));
}

#[test]
fn a_follow_up_cast_from_a_nested_skill_still_runs() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(312451011),
                    from_uid: Some(10),
                    duration: Some(3),
                    ..Default::default()
                }],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451115], []);
    // A nested cast never completes an action, so nothing is held for it.
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312451115,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let result = run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        crate::engine::skill::action::SkillModifiers::default(),
    )
    .unwrap();

    let kicks = result
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                crate::engine::event::payload::BattleEvent::SkillAction(action)
                    if action.skill_id == 312451011
                        && action.phase == crate::engine::skill::action::SkillPhase::Immediate
            )
        })
        .count();
    assert_eq!(kicks, 1);
}

#[test]
fn an_attacked_targets_attack_start_reaction_runs_before_the_attacks_own_effects() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![109380003],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 109380003], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;
    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();
    let position = |matches: fn(&crate::engine::event::payload::BattleEvent) -> bool| {
        result
            .events
            .iter()
            .position(matches)
            .expect("the event is published")
    };
    // "When being actively attacked, the attack counts as a Stronger Afflatus attack."
    let stronger_afflatus = position(|event| {
        matches!(
            event,
            crate::engine::event::payload::BattleEvent::BuffAdded(buff)
                if buff.target_uid == 10 && buff.buff_id == 109380006
        )
    });
    let attack_effects = position(|event| {
        matches!(
            event,
            crate::engine::event::payload::BattleEvent::SkillAction(action)
                if action.skill_id == 31090111
                    && action.phase == crate::engine::skill::action::SkillPhase::Immediate
        )
    });
    assert!(stronger_afflatus < attack_effects);
}

#[test]
fn a_later_slot_sees_the_changes_of_an_earlier_slot_in_the_same_phase() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(109380001),
                        from_uid: Some(-1),
                        duration: Some(3),
                        ..Default::default()
                    }],
                    ..entity(10)
                },
                entity(11),
            ],
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [109380001], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 109380001,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();

    // "After hitting a target afflicted with [Vacant], ... removes [Vacant] from them;
    // otherwise, inflicts [Vacant] on the target."
    assert!(!managers.buff.has_active_buff_id(10, 109380001));
    assert!(managers.buff.has_active_buff_id(11, 109380001));
    assert!(!managers.buff.has_active_buff_id(10, 109380005));
}

#[test]
fn a_riposte_completes_as_an_ally_action() {
    let result = attack_with_passives(
        Vec::new(),
        vec![31050141],
        Vec::new(),
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::Riposte),
    );

    // Flutterpage: "After any ally takes an action, gains 1 stack of [Gust]".
    assert!(result.events.iter().any(|event| matches!(
        event,
        crate::engine::event::payload::BattleEvent::BuffAdded(gust)
            if gust.target_uid == 11 && gust.buff_id == 31050111
    )));
}
