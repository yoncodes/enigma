use super::*;

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
            .passes()
            .flat_map(|pass| &pass.skills)
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
                catalog.rule_limit_owner(passive_skill, slot_index),
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
fn restrained_ultimate_consumes_bedrock_before_damage_once_per_round() {
    crate::test_support::init_config();
    let fight = || Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                career: Some(3),
                current_hp: Some(10_000),
                ex_point: Some(5),
                ex_skill: Some(30060133),
                skill_group1: vec![30060111],
                attr: Some(HeroAttribute {
                    attack: Some(1),
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
                team_type: Some(2),
                career: Some(8),
                weak_careers: vec![3],
                current_hp: Some(1_000_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000_000),
                    ..Default::default()
                }),
                passive_skill: vec![109360013],
                buffs: vec![BuffInfo {
                    uid: Some(100),
                    buff_id: Some(109360005),
                    from_uid: Some(-1),
                    layer: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let action = |skill_id| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = SkillExecutionMode::Active;
        invocation
    };
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let context = TargetContext {
        current_round: 1,
        ..Default::default()
    };

    let normal_fight = fight();
    let normal_pool = TargetPool::from_fight(&normal_fight);
    let mut normal_managers = BattleManagers::seeded(&normal_fight);
    run_action(
        &mut normal_managers,
        &normal_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        action(30060111),
    )
    .unwrap();
    assert_eq!(normal_managers.buff.buff_id_amount(-1, 109360005), 7);

    let ultimate_fight = fight();
    let ultimate_pool = TargetPool::from_fight(&ultimate_fight);
    let mut ultimate_managers = BattleManagers::seeded(&ultimate_fight);
    let ultimate_result = run_action_with_cost(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        Some(ExPointCommand::Spend(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid: 10,
            target_uid: 10,
            delta: -5,
            config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        })),
        action(30060133),
    )
    .unwrap();
    assert_eq!(ultimate_managers.buff.buff_id_amount(-1, 109360005), 0);

    fn ordered_changes<'a>(
        frame: &'a crate::engine::runtime::record::SemanticFrame,
        changes: &mut Vec<&'a BattleChange>,
    ) {
        for item in &frame.items {
            match item {
                crate::engine::runtime::record::FrameItem::Change(change) => changes.push(change),
                crate::engine::runtime::record::FrameItem::Child(child) => {
                    ordered_changes(child, changes)
                }
                crate::engine::runtime::record::FrameItem::Cue(_) => {}
            }
        }
    }
    let mut changes = Vec::new();
    for frame in &ultimate_result.frames {
        ordered_changes(frame, &mut changes);
    }
    let bedrock_removal = changes
        .iter()
        .position(|change| {
            matches!(
                change,
                BattleChange::Buff(change)
                    if change.change.removed.iter().any(|removed| {
                        removed.buff.buff_id == Some(109360005)
                    })
            )
        })
        .unwrap();
    let incoming_damage = changes
        .iter()
        .position(|change| {
            matches!(
                change,
                BattleChange::Hp(change)
                    if change.target_uid == -1 && change.damage.is_some()
            )
        })
        .unwrap();
    assert!(bedrock_removal < incoming_damage);

    run(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 109360005,
                amount: Some(10),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        )))],
    )
    .unwrap();
    run_action(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        action(30060133),
    )
    .unwrap();
    assert_eq!(ultimate_managers.buff.buff_id_amount(-1, 109360005), 10);
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
    assert!(
        fear < attacked && attacked < combustion_cleanup && combustion_cleanup < shock_wave,
        "fear={fear} attacked={attacked} combustion_cleanup={combustion_cleanup} shock_wave={shock_wave}"
    );
}
