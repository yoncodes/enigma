use super::*;

#[test]
fn self_destructing_attack_finishes_its_damage_phase() {
    crate::test_support::init_config();
    let entity = |uid, hp, attack| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        battle_id: Some(1),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 10_000, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 1_000)],
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
                BehaviorSpec::new(30006, "LostLife"),
                vec![0, AttrId::CurrentHp as i32, 1_000],
                Vec::new(),
            ),
            TargetRequest {
                code: crate::engine::skill::target::request::SOURCE_TARGET_CODE,
                raw: Vec::new(),
            },
        )],
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 202);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            battle_id: 1,
            ..Default::default()
        },
        [],
        invocation,
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.hp.current(10), 9_000);
    assert!(managers.terminal_outcome().is_some());
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
            technic: Some(if uid > 0 { 1_000 } else { 0 }),
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
            .filter_map(|(index, event)| {
                let crate::engine::event::payload::BattleEvent::Hit(hit) = event else {
                    return None;
                };
                (hit.skill_id == 312451011).then_some((index, hit.assassinate, hit.resolved_damage))
            })
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
                .all(|(kick, _, _)| completed.is_some_and(|completed| completed < *kick))
        );
        kicks
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

    assert!(sparta_kicks(Vec::new()).is_empty());
    let generated = sparta_kicks(vec![kick_gash.clone()]);
    assert_eq!(generated.len(), 1);
    assert!(generated[0].1);
    let preexisting = sparta_kicks(vec![
        kick_gash.clone(),
        BuffInfo {
            uid: Some(32),
            buff_id: Some(31240121),
            from_uid: Some(10),
            layer: Some(1),
            ..Default::default()
        },
    ]);
    assert_eq!(preexisting[0].2, generated[0].2);
    assert_eq!(sparta_kicks(vec![kick_gash, arrow_gash]).len(), 2);
}

#[test]
fn held_shell_cast_follows_the_first_recast_attack() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            technic: Some(if uid > 0 { 1_000 } else { 0 }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10),
                FightEntityInfo {
                    passive_skill: vec![31090144],
                    ..entity(11)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![
                    BuffInfo {
                        uid: Some(30),
                        buff_id: Some(312451011),
                        from_uid: Some(10),
                        duration: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(31),
                        buff_id: Some(312451021),
                        from_uid: Some(10),
                        duration: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(32),
                        buff_id: Some(31090112),
                        from_uid: Some(11),
                        layer: Some(16),
                        ..Default::default()
                    },
                ],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451115, 31090144], []);
    crate::engine::mechanic::shell::execute(
        &mut managers,
        ShellCommand::AccumulateAndUseSkill {
            rule: ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill")),
            source_uid: 11,
            target_uid: -1,
            threshold: 7,
            delta: 6,
            skill_id: 31090114,
        },
    )
    .unwrap();
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312451115,
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

    let fury = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(312451115))
        .expect("Fury of Ares projects a step");
    let kicks = fury
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter(|step| {
            step.act_id == Some(312451011)
                && step.act_type == Some(sonettobuf::fight_step::ActType::Skill as i32)
        })
        .collect::<Vec<_>>();

    assert_eq!(kicks.len(), 2);
    assert!(kicks[0].act_effect.iter().any(|effect| {
        effect
            .fight_step
            .as_ref()
            .is_some_and(|step| step.act_id == Some(31090144))
    }));
    assert!(kicks[1..].iter().all(|kick| {
        !kick.act_effect.iter().any(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_id == Some(31090144))
        })
    }));
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
                    passive_skill: vec![31090144, 434111],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31090111),
                            from_uid: Some(10),
                            layer: Some(15),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31050145),
                            from_uid: Some(11),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(22),
                            buff_id: Some(109380006),
                            from_uid: Some(-1),
                            ..Default::default()
                        },
                    ],
                    ..entity(10)
                },
                // Flutterpage gains Gust after any ally takes an action.
                FightEntityInfo {
                    passive_skill: vec![31050141],
                    power_infos: vec![sonettobuf::PowerInfo {
                        power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
                        num: Some(2),
                        max: Some(5),
                    }],
                    ..entity(11)
                },
            ],
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
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31090111, 31090144, 31050141, 434111, 109380003],
        [],
    );
    // Six deployments or retrievals so far; the attack's deployment is the seventh.
    crate::engine::mechanic::shell::execute(
        &mut managers,
        ShellCommand::AccumulateAndUseSkill {
            rule: ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill")),
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
    invocation.target = SkillTarget::Explicit(11);
    invocation.mode = SkillExecutionMode::Active;
    invocation.card_index = 1;

    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(31090114, 10, [true]);
    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
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
    assert_eq!(attack.to_id, Some(11));
    assert!(managers.hp.current(-1) < 100_000);
    assert_eq!(managers.eureka.get(11, 1).current, 0);
    assert!(managers.buff.buff_id_amount(-1, 31090112) > 0);
    assert_eq!(managers.buff.buff_id_amount(11, 31090112), 0);
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
    assert_eq!(last_child.to_id, Some(11));
    let attached = last_child
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| step.act_id == Some(31090114) && step.to_id == Some(-1))
        .expect("the shell cast stays attached to its triggering action");
    let direct_hit = attached
        .act_effect
        .iter()
        .find(|effect| effect.hurt_info.is_some())
        .expect("the attached cast deals direct damage");
    assert_eq!(
        direct_hit.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Crit as i32)
    );
    assert_eq!(direct_hit.effect_num, Some(1_614));
    assert!(
        attached
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|step| step.act_id == Some(31050141)),
        "the attached cast still publishes its own ally action"
    );
    assert!(
        attached
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|step| step.act_id == Some(31050145)),
        "the attached extra action activates Wave of Sheets"
    );
    assert!(
        attached
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|step| step.act_id == Some(434111)),
        "the attached cast keeps its configured extra-skill kind"
    );
    assert!(
        attached
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .all(|step| step.act_id != Some(109380003)),
        "the attached cast is not a player-selected active attack"
    );
}

#[test]
fn shell_necklace_cast_follows_action_reaction_after_non_action() {
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
                FightEntityInfo {
                    passive_skill: vec![31050141],
                    ..entity(11)
                },
                FightEntityInfo {
                    passive_skill: vec![312301701],
                    ..entity(12)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(229701),
                    from_uid: Some(12),
                    layer: Some(2),
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
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31090111, 31090144, 312301701, 31050141],
        [],
    );
    crate::engine::mechanic::shell::execute(
        &mut managers,
        ShellCommand::AccumulateAndUseSkill {
            rule: ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill")),
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
    invocation.card_index = 3;
    invocation.target = SkillTarget::Explicit(11);
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
    assert!(!children.iter().any(|step| step.act_id == Some(31090144)));
    let reuse_index = children
        .iter()
        .position(|step| step.act_id == Some(312301701))
        .expect("Ezio reacts to the marked target");
    let flutter_index = children
        .iter()
        .position(|step| step.act_id == Some(31050141))
        .expect("Flutterpage also reacts to the attack");
    assert!(flutter_index < reuse_index);
    let reuse = children[reuse_index];
    let follow_up = reuse
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| step.act_id == Some(312301711))
        .expect("Ezio performs the follow-up action");
    let shell = follow_up
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter(|step| step.act_id == Some(31090144))
        .collect::<Vec<_>>();
    assert_eq!(shell.len(), 1);
    assert_eq!(shell[0].to_id, Some(-1));
    assert_eq!(
        shell[0]
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .filter(|step| step.act_id == Some(31090114))
            .count(),
        1
    );
}

#[test]
fn deferred_caster_frame_targets_the_exact_action_it_joins() {
    let mut frames = Vec::new();
    let action = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 12,
            skill_id: 200,
            card_index: 0,
            target_uid: Some(-1),
        },
        FrameTrigger::Active,
    );
    let caster = FrameOwner::Skill {
        source_uid: 10,
        skill_id: 100,
        card_index: 0,
        target_uid: Some(11),
    };

    assert_eq!(
        join_caster_to_action(&frames, &action, caster.clone()),
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            card_index: 0,
            target_uid: Some(-1),
        }
    );

    let conduit_action = push_root(
        &mut frames,
        FrameOwner::ConduitSkill {
            source_uid: 12,
            skill_id: 300,
            device_index: 1,
            target_uid: Some(-2),
        },
        FrameTrigger::Active,
    );
    assert_eq!(
        join_caster_to_action(&frames, &conduit_action, caster),
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            card_index: 0,
            target_uid: Some(-2),
        }
    );
}

#[test]
fn shell_necklace_cast_releases_inside_the_following_riposte() {
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
            entitys: vec![FightEntityInfo {
                passive_skill: vec![31090144],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31090111),
                    from_uid: Some(10),
                    layer: Some(15),
                    ..Default::default()
                }],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1),
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(2292031),
                        from_uid: Some(-2),
                        duration: Some(3),
                        ..Default::default()
                    }],
                    ..entity(-2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 31090144, 312301621], []);
    crate::engine::mechanic::shell::execute(
        &mut managers,
        ShellCommand::AccumulateAndUseSkill {
            rule: ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill")),
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
    let riposte = children
        .iter()
        .find(|step| step.act_id == Some(2292031))
        .expect("the attack causes the ally riposte");
    assert!(!children.iter().any(|step| step.act_id == Some(31090144)));
    assert!(riposte.act_effect.iter().any(|effect| {
        effect.fight_step.as_ref().is_some_and(|step| {
            step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|step| step.act_id == Some(31090144))
            })
        })
    }));
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

pub(super) fn attack_with_passives(
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
fn a_direct_cast_stays_with_its_action_when_an_ally_reaction_starts_an_action() {
    let result = attack_with_passives(
        vec![400],
        vec![500],
        vec![
            direct_use_passive(400, 402, 103, 401),
            ParsedSkillEffect {
                skill_id: 401,
                slots: Vec::new(),
            },
            direct_use_passive(500, 212, 101, 501),
            ParsedSkillEffect {
                skill_id: 501,
                slots: Vec::new(),
            },
        ],
        None,
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

    assert!(children.iter().any(|step| step.act_id == Some(500)));
    let direct = children
        .iter()
        .find(|step| step.act_id == Some(400))
        .expect("the direct cast stays under its passive");
    assert!(direct.act_effect.iter().any(|effect| {
        effect
            .fight_step
            .as_ref()
            .is_some_and(|step| step.act_id == Some(401))
    }));
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
