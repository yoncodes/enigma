use super::*;

#[test]
fn version_seven_card_cleanup_follows_deferred_round_start_settlement() {
    init_config();
    let entity = |uid, team_type, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(current_hp),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![100],
                ..entity(10, 1, 100)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(100),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(60015, "Kill"), Vec::new(), Vec::new()),
        TargetRequest {
            code: 202,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();
    let position = |phase| {
        round
            .frames
            .iter()
            .position(|frame| frame.owner == FrameOwner::RoundPhase(phase))
            .expect("round phase is present")
    };

    assert!(position(RoundPhase::RoundStartSettlement) < position(RoundPhase::EntityCardCleanup));
    assert!(position(RoundPhase::RoundStartSync) < position(RoundPhase::EntityCardCleanup));
    assert_eq!(managers.hp.current(-1), 0);
    assert!(managers.card.ai_queue().is_empty());

    let steps = crate::engine::packet::timeline::project(&round.frames).unwrap();
    assert!(steps.iter().any(|step| skill_contains_effect(
        step,
        100,
        sonettobuf::effect_type_enum::EffectType::Dead as i32,
        -1,
    )));
    assert!(!steps.iter().any(|step| skill_contains_effect(
        step,
        100,
        sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32,
        -1,
    )));
    let cleanup = steps
        .iter()
        .position(|step| {
            contains_effect(
                step,
                sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32,
                -1,
            )
        })
        .expect("dead defender cards are cleaned in a standalone step");
    let deck_count = steps
        .iter()
        .position(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
            })
        })
        .expect("deck count follows round-start cleanup");
    assert!(cleanup < deck_count);
}

#[test]
fn defender_round_start_death_cleans_its_cards_in_the_death_action() {
    init_config();
    let fight = Fight {
        version: Some(7),
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
                passive_skill: vec![101],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(100),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(60015, "Kill"), Vec::new(), Vec::new()),
        TargetRequest {
            code: crate::engine::skill::target::request::SOURCE_TARGET_CODE,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 101,
        slots: vec![slot],
    });

    let round = run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        &[],
        None,
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert!(managers.card.ai_queue().is_empty());
    let steps = crate::engine::packet::timeline::project(&round.frames).unwrap();
    assert!(steps.iter().any(|step| skill_contains_effect(
        step,
        101,
        sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32,
        -1,
    )));
}

#[test]
fn action_queue_stops_when_the_configured_win_target_dies() {
    init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(hp),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        battle_id: Some(301110),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 30111001, 1), entity(-4, 30111002, 1_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 30111005,
                parameters: [1_000, 0],
            },
        })
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        managers
            .entity
            .passive_skills(-1)
            .into_iter()
            .flatten()
            .copied(),
        std::iter::empty(),
    );
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(999, 1_000);

    run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            AiSkillChoice {
                source_uid: 10,
                skill_id: 999,
                target_uid: -1,
            },
            AiSkillChoice {
                source_uid: 10,
                skill_id: 999,
                target_uid: -4,
            },
        ],
        &[],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.hp.current(-4), 1_000);
}

#[test]
fn lethal_skill_cost_settles_before_damage_and_stops_later_behaviors() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(10),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(100),
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [119381126], []);

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 119381126,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find_map(|step| find_skill(step, 119381126))
        .unwrap();
    let death = skill
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32)
                && effect.target_id == Some(-1)
        })
        .unwrap();
    let damage = skill
        .act_effect
        .iter()
        .position(|effect| {
            matches!(
                effect.effect_type,
                Some(kind)
                    if kind == sonettobuf::effect_type_enum::EffectType::Damage as i32
                        || kind == sonettobuf::effect_type_enum::EffectType::Crit as i32
            ) && effect.target_id == Some(10)
        })
        .unwrap();

    assert!(death < damage);
    assert!(!skill.act_effect.iter().any(|effect| {
        effect
            .buff
            .as_ref()
            .and_then(|buff| buff.buff_id)
            .is_some_and(|buff_id| matches!(buff_id, 4101 | 5112))
    }));
}

#[test]
fn prior_source_death_does_not_suppress_after_damage_behaviors() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(0),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [119381126], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 119381126,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.phase = Some(crate::engine::skill::action::SkillPhase::AfterDamage);
    invocation.target = SkillTarget::Explicit(10);
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.record_targets([10]);
    execution.record_attacks([10]);

    let emission = crate::engine::runtime::skill::emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &crate::engine::runtime::skill::SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(emission.ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
            if matches!(grant.buff_id, 4101 | 5112)
    )));
}
