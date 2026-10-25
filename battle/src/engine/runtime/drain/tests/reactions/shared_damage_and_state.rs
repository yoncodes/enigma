use super::*;

#[test]
fn mane_mass_attack_orders_shared_damage_before_hit_passives_and_direct_hit() {
    crate::test_support::init_config();
    let entity = |position, uid, passive_skill, buffs| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        team_type: Some(1),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        passive_skill,
        buffs,
        ..Default::default()
    };
    let share_buff = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(31090121),
        from_uid: Some(13),
        layer: Some(3),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    current_hp: Some(2_000),
                    ..entity(1, 10, vec![312301533], vec![share_buff(50)])
                },
                entity(2, 11, Vec::new(), vec![share_buff(51)]),
                entity(3, 12, Vec::new(), vec![share_buff(52)]),
                FightEntityInfo {
                    equips: vec![sonettobuf::EquipRecord {
                        equip_id: Some(1541),
                        refine_lv: Some(1),
                        ..Default::default()
                    }],
                    ..entity(4, 13, vec![434111], vec![share_buff(53)])
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                team_type: Some(2),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![109380006],
                power_infos: vec![
                    PowerInfo {
                        power_id: Some(9),
                        num: Some(0),
                        max: Some(4),
                    },
                    PowerInfo {
                        power_id: Some(10),
                        num: Some(100),
                        max: Some(100),
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
    assert_eq!(
        managers.entity.passive_source(13, 434111),
        crate::engine::entity::passive::PassiveSourceKind::Psychube
    );
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [109380001, 109380006, 312301533, 434111],
        [],
    );
    let attacked = crate::engine::skill::subscriber::for_compiled_event(
        &pool,
        &managers,
        &catalog,
        crate::engine::event::kind::EventKind::TargetAttacked,
    )
    .unwrap()
    .skills
    .into_iter()
    .find(|subscriber| subscriber.skill_id == 434111)
    .unwrap();
    assert_eq!(
        attacked.key.timing,
        crate::engine::event::subscription::ReactionTiming::AfterHitObservers
    );
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 109380001,
    }
    .into();
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

    fn collect_skills(step: &sonettobuf::FightStep, skills: &mut Vec<i32>) {
        if let Some(skill_id @ (434111 | 109380006 | 312301533)) = step.act_id {
            skills.push(skill_id);
        }
        for effect in &step.act_effect {
            if let Some(child) = effect.fight_step.as_ref() {
                collect_skills(child, skills);
            }
        }
    }
    let mut skills = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect_skills(&step, &mut skills);
    }
    assert_eq!(skills, vec![312301533, 434111, 109380006, 434111]);
}

#[test]
fn current_hp_damage_reads_each_target_after_prior_shared_damage_settles() {
    crate::test_support::init_config();
    let ally = |position, uid, buff_uid| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        team_type: Some(1),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31090121),
            from_uid: Some(13),
            layer: Some(3),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                ally(1, 10, 50),
                ally(2, 11, 51),
                ally(3, 12, 52),
                ally(4, 13, 53),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                team_type: Some(2),
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30014, "OriginDamage"),
                vec![1, AttrId::CurrentHp as i32, 400],
                Vec::new(),
            ),
            TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
        )],
    });
    catalog.insert_logic_target(100, 202);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 100,
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

    for uid in [10, 11, 12, 13] {
        assert_eq!(managers.hp.current(uid), 6_561);
    }
}

#[test]
fn carrier_death_pipeline_returns_deployed_shells_to_the_caster() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(10_000),
                passive_skill: vec![31090141],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090112),
                    layer: Some(3),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    fight.attacker.as_mut().unwrap().entitys[0].current_hp = Some(0);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: -1,
            target_uid: 10,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(-1, 31090112), 0);
    assert_eq!(managers.buff.buff_id_amount(10, 31090111), 3);
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
fn active_ally_reaction_without_parent_keeps_the_action_target() {
    crate::test_support::init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
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
                entity(10, 3149),
                FightEntityInfo {
                    passive_skill: vec![31430151],
                    ..entity(30, 3143)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
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
            skill_id: 31490111,
            target_uid: 30,
            target_uids: vec![30],
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            mode: SkillExecutionMode::Active,
            ..Default::default()
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let reaction = steps
        .iter()
        .find_map(|step| find_step(step, 31430151))
        .unwrap();
    assert_eq!(reaction.to_id, Some(30));
    assert!(reaction.act_effect.iter().any(|effect| {
        effect.target_id == Some(30)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(31430151))
    }));
}

#[test]
fn rejected_seal_triggers_the_configured_moxie_fallback() {
    crate::test_support::init_config();
    let fight = |immune: bool| Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ex_skill: Some(30200135),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                ex_point: Some(5),
                ex_point_max: Some(5),
                attr: Some(HeroAttribute {
                    defense: Some(1_000),
                    hp: Some(100_000),
                    ..Default::default()
                }),
                buffs: immune
                    .then_some(BuffInfo {
                        buff_id: Some(5140006),
                        uid: Some(20),
                        from_uid: Some(-1),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let execute = |fight: &Fight| {
        let pool = TargetPool::from_fight(fight);
        let mut managers = BattleManagers::seeded(fight);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 30200135,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = SkillExecutionMode::Active;
        let result = run(
            &mut managers,
            &pool,
            crate::engine::skill::effect::catalog::global(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        (managers, result)
    };

    let (immune_managers, immune_result) = execute(&fight(true));
    assert!(!immune_managers.buff.has_buff_id_or_type(-1, 4007));
    assert_eq!(immune_managers.ex_point.get(-1), 3);
    assert!(immune_result.events.iter().any(|event| matches!(
        event,
        BattleEvent::BuffRejected(rejected)
            if rejected.target_uid == -1
                && rejected.buff_id == 720202
                && rejected.type_id == 4007
    )));
    let immune_steps = crate::engine::packet::timeline::project(&immune_result.frames).unwrap();
    assert!(
        immune_steps
            .iter()
            .any(|step| step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Buffreject as i32)
                    && effect.target_id == Some(-1)
            }))
    );

    let (normal_managers, _) = execute(&fight(false));
    assert!(normal_managers.buff.has_buff_id_or_type(-1, 4007));
    assert_eq!(normal_managers.ex_point.get(-1), 5);
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
            None,
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
        resolved_damage: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
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
        .passes()
        .flat_map(|pass| &pass.skills)
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
