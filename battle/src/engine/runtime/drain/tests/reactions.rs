use super::*;

#[test]
fn assist_boss_attack_passive_resolves_from_the_derived_skill_cast_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![12_720_012],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: -1,
        skill_id: 370_001_002,
        target_uid: -2,
        target_uids: vec![-2, -3],
        attacked_target_uids: vec![-2, -3],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: -1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 2,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(managers.hp.current(-3) < 10_000);
}

#[test]
fn random_additional_target_passive_expands_the_configured_attack() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![370_002_190],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_condition_random_choices(vec![
        crate::engine::runtime::determinism::ConditionRandomChoice {
            skill_id: 370_001_002,
            opcode: 552_203,
            roll: 499,
        },
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 370_001_002,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-2);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(
        managers.hp.current(-3) < 10_000,
        "marker={} frames={:#?}",
        managers.buff.has_buff_id(-1, 370_002_190),
        result.frames
    );
    assert!(!managers.buff.has_buff_id(-1, 370_002_190));
}

#[test]
fn active_skill_publishes_exact_phase_to_its_psychube_passive() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3107),
                    team_type: Some(1),
                    career: Some(6),
                    current_hp: Some(100),
                    skill_group1: vec![31070111],
                    passive_skill: vec![435011],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3074),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![2270001],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                career: Some(6),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31070111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.card_index = 1;

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31070111));
    assert!(managers.buff.has_buff_id(10, 435011));
    assert!(!managers.buff.has_buff_id(11, 90071));
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31070111))
            .and_then(|buff| buff.act_common_params.as_deref()),
        Some("1003#0")
    );

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let root = steps
        .iter()
        .find(|step| step.act_id == Some(31070111))
        .expect("the active skill owns its packet frame");
    assert_eq!(root.to_id, Some(-1));
    assert_eq!(
        root.act_effect
            .iter()
            .take(3)
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::None as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32),
        ]
    );
    let passive = root.act_effect[2]
        .fight_step
        .as_ref()
        .expect("the psychube reaction stays nested under the active skill");
    assert_eq!(passive.act_id, Some(435011));
    assert_eq!(passive.from_id, Some(10));
    assert_eq!(passive.to_id, Some(-1));
}

#[test]
fn reactive_skill_frame_targets_the_other_team_of_a_hit() {
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
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(100, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -2,
        skill_id: 100,
        amount: 50,
        shield_absorbed: 0,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });

    assert_eq!(reaction_counterparty(&pool, &event, -2), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, -3), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, 10), Some(-2));
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::Counterparty,
        ),
        Some(10)
    );
}

#[test]
fn attack_consumption_keeps_first_hit_entity_order() {
    let hit = |source_uid, target_uid| {
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100, "SkillDamage"),
            },
            source_uid,
            target_uid,
            skill_id: 100,
            amount: 50,
            shield_absorbed: 0,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })
    };
    let events = [hit(10, -2), hit(10, -1), hit(11, -2)];

    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.source_uid),
        [10, 11]
    );
    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.target_uid),
        [-2, -1]
    );
}

#[test]
fn allied_action_observer_keeps_the_triggering_action_target() {
    let event = BattleEvent::AllyAction(ActionEvent {
        source_uid: 10,
        target_uid: -2,
        skill_id: 100,
        skill_slot: 1,
        is_attack: true,
        rank: 1,

        skill_type: 0,
        effect_tag: 1,
        additional_moxie: 0,
        extra_skill_kind: 0,
        assassinate: false,
        ..Default::default()
    });

    assert_eq!(
        reaction_counterparty(&TargetPool::default(), &event, 99),
        Some(-2)
    );
}

#[test]
fn eureka_threshold_reaction_observes_the_gain_from_the_same_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                passive_skill: vec![30660143, 30660193],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
                }],
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::AllyAction(ActionEvent {
            source_uid: 10,
            skill_id: 30660111,
            target_uid: -1,
            target_uids: vec![-1],
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            ..Default::default()
        }),
    )
    .unwrap();

    let deltas = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::EurekaChanged(change) if change.target_uid == 10 => {
                Some(change.applied_delta)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(deltas, [1, -5]);
    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 0);
    assert!(managers.hp.current(-1) < 10_000);
}

#[test]
fn after_hit_passive_uses_the_active_skills_successful_buff_additions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![200],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut passive_slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60059, "AddBurnBySkillAddBurnCount"),
            vec![4150001],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    passive_slot.conditions = vec![ParsedCondition {
        opcode: 210,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillActionAfterHit),
        raw_args: Vec::new(),
    }];
    passive_slot.compiled_route = ConditionRoute::compile(&passive_slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![4150001], Vec::new()),
            TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
        )],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![passive_slot],
    });

    run(
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

    assert_eq!(managers.buff.max_id_or_type_layer(10, 4150001), 3);
    assert!(managers.buff.has_buff_id(-1, 4150001));
    assert!(managers.buff.has_buff_id(-2, 4150001));
    assert!(managers.buff.has_buff_id(-3, 4150001));
}

#[test]
fn eureka_reaction_frame_stays_owned_by_the_subscriber() {
    let event = BattleEvent::EurekaChanged(crate::engine::event::payload::EurekaChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "SpendEureka"),
        },
        source_uid: 10,
        target_uid: 20,
        power_id: EUREKA_RESOURCE_ID,
        before: 3,
        requested_delta: -2,
        applied_delta: -2,
        after: 1,
        overflow: 0,
    });

    assert_eq!(
        reaction_skill_target(
            &TargetPool::default(),
            &event,
            99,
            crate::engine::skill::condition::registry::ReactionFrameTarget::Owner,
        ),
        Some(99)
    );
}

#[test]
fn event_emitted_skill_starts_a_fresh_cast_with_its_explicit_target() {
    crate::test_support::init_config();
    let entity = |uid, model_id, position, career, current_hp, max_hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(position),
        career: Some(career),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(max_hp),
            ..Default::default()
        }),
        ..Default::default()
    };
    let facade = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(530000111),
        layer: Some(2),
        count: Some(1),
        ..Default::default()
    };
    let mut ally = entity(20, 3114, 2, 1, 100, 100);
    ally.skill_group1 = vec![31140111, 31140112, 31140113];
    ally.skill_group2 = vec![31140121, 31140122, 31140123];
    let mut pickles = entity(30, 3063, 3, 1, 100, 100);
    pickles.passive_skill = vec![30630151];
    pickles.skill_group1 = vec![30630111, 30630112, 30630113];
    pickles.skill_group2 = vec![30630121, 30630122, 30630123];
    let mut first = entity(-1, 30110801, 1, 4, 50, 100);
    first.buffs = vec![facade(101)];
    let mut second = entity(-2, 30110802, 2, 4, 100, 100);
    second.buffs = vec![facade(102)];
    let mut third = entity(-3, 30110803, 3, 4, 100, 100);
    third.buffs = vec![facade(103)];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![ally, pickles],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![first, second, third],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_event(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::AllyAction(ActionEvent {
            source_uid: 20,
            skill_id: 31140131,
            target_uid: -1,
            skill_slot: 3,
            is_attack: false,
            rank: 1,
            skill_type: 0,
            effect_tag: 4,
            additional_moxie: 0,
            extra_skill_kind: 0,
            assassinate: false,
            ..Default::default()
        }),
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(-1, 530000111));
    assert!(!managers.buff.has_buff_id(-2, 530000111));
    assert!(!managers.buff.has_buff_id(-3, 530000111));
}

#[test]
fn target_attacked_passive_and_be_attacked_buff_act_share_one_hit_payload() {
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                passive_skill: vec![530000151],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(530000111),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30620111),
                        layer: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(100, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 100,
        amount: 50,
        shield_absorbed: 0,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(-1, 530000111));
    assert_eq!(managers.buff.snapshot(-1, 21).unwrap().layer, Some(0));
    assert_eq!(managers.ex_point.get(10), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(|step| step.act_id == Some(530000151)));
    assert!(steps.iter().any(|step| step.act_id == Some(30620111)));
}

#[test]
fn entity_defeat_passive_executes_each_configured_sibling_slot() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                passive_skill: vec![30865186],
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30860113),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        defense: Some(500),
                        mdefense: Some(500),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        defense: Some(500),
                        mdefense: Some(500),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
        source_uid: 10,
        target_uid: -1,
    });

    let dispatched = dispatcher::dispatch_event(
        &pool.runtime_view(&managers),
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &event,
    )
    .unwrap();
    let mut slots = dispatched
        .skills
        .iter()
        .filter(|(subscriber, _)| subscriber.skill_id == 30865186)
        .filter_map(|(subscriber, _)| subscriber.slot_index)
        .collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(slots, vec![4, 5]);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert_eq!(managers.buff.snapshot(10, 20).unwrap().layer, Some(4));
}

#[test]
fn lucy_entity_defeat_follow_up_respects_its_configured_round_limit() {
    crate::test_support::init_config();

    let fires = |passive_skill, round_limit| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    passive_skill: vec![passive_skill],
                    attr: Some(HeroAttribute {
                        attack: Some(100),
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        team_type: Some(2),
                        current_hp: Some(0),
                        attr: Some(HeroAttribute {
                            hp: Some(10_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(-2),
                        team_type: Some(2),
                        current_hp: Some(1_000_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000_000),
                            defense: Some(100),
                            mdefense: Some(100),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let mut managers = BattleManagers::seeded(&fight);
        let event = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        });
        let dispatched = dispatcher::dispatch_event(
            &pool.runtime_view(&managers),
            &managers,
            &catalog,
            &mut RoundDeterminism::default(),
            &event,
        )
        .unwrap();
        let effect = catalog.get(passive_skill).unwrap();
        let subscriber = dispatched
            .skills
            .iter()
            .map(|(subscriber, _)| subscriber)
            .find(|subscriber| {
                subscriber.skill_id == passive_skill
                    && subscriber
                        .slot_index
                        .is_some_and(|slot| effect.slots[slot].round_limit == round_limit)
            })
            .unwrap();
        let slot_index = subscriber.slot_index.unwrap();
        let slot = &effect.slots[slot_index];
        assert_eq!(slot.round_limit, round_limit);
        let can_fire = |managers: &BattleManagers| {
            managers.can_fire_rule(
                10,
                passive_skill,
                slot_index,
                subscriber.key.definition,
                slot.limit,
                slot.round_limit,
            )
        };

        for _ in 0..round_limit {
            assert!(can_fire(&managers));
            run_event(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
                event.clone(),
            )
            .unwrap();
        }

        assert!(!can_fire(&managers));
        for _ in 0..2 {
            run_event(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
                event.clone(),
            )
            .unwrap();
        }
        assert!(!can_fire(&managers));
        managers.begin_round();
        assert!(can_fire(&managers));
        run_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            event,
        )
        .unwrap();
        assert_eq!(can_fire(&managers), round_limit > 1);
    };

    fires(30865171, 2);
    fires(30865175, 4);
    fires(30865186, 4);
}

#[test]
fn gorgon_death_kills_tentacles_and_exposes_the_core() {
    crate::test_support::init_config();
    let entity = |uid, model_id, position, hp, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(position),
        team_type: Some(2),
        current_hp: Some(hp),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 150401, 1, 10_000, vec![114200141]),
                entity(-2, 150402, 2, 10_000, vec![]),
                entity(-3, 150403, 3, 10_000, vec![]),
            ],
            sp_entitys: vec![entity(-4, 150404, 4, 10_000, vec![114200143])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers.hp.lose(-1, 10_000, 10);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-2), 0);
    assert_eq!(managers.hp.current(-3), 0);
    assert_eq!(managers.hp.current(-1), 5_000);
    assert!(managers.buff.has_buff_id(-4, 11410082));

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -2,
        }),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 5_000);
}

#[test]
fn active_skill_publishes_hits_between_after_damage_and_after_hit_rows() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31280113),
                        layer: Some(50),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(33),
                        buff_id: Some(4150002),
                        count: Some(1),
                        layer: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
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
                passive_skill: vec![530000151],
                buffs: vec![
                    BuffInfo {
                        uid: Some(31),
                        buff_id: Some(31280111),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(32),
                        buff_id: Some(530000111),
                        layer: Some(2),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31280111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = &steps
        .iter()
        .find(|step| step.act_id == Some(31280111))
        .unwrap()
        .act_effect;
    let fear_act_info = effects
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
                && effect.target_id == Some(-1)
        })
        .unwrap();
    let fear_delete = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280111)
        })
        .unwrap();
    let fear = effects
        .iter()
        .rposition(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280111))
        .unwrap();
    let attacked = effects
        .iter()
        .position(|effect| {
            effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(530000151)
        })
        .unwrap();
    let combustion_cleanup = effects
        .iter()
        .position(|effect| effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(4150002))
        .unwrap();
    let shock_wave = effects
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280113))
        .unwrap();

    assert!(fear_act_info < fear_delete);
    assert!(fear < attacked && attacked < combustion_cleanup && combustion_cleanup < shock_wave);
}
