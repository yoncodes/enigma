use super::*;

#[test]
fn player_and_marked_monster_reserves_promote_after_the_action() {
    init_config();
    let entity = |uid, team_type, position, hp, attack| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(hp),
        position: Some(position),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 100, 100), entity(12, 1, 2, 100, 1)],
            sub_entitys: vec![entity(11, 1, -1, 100, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 1, 1)],
            sub_entitys: vec![entity(-2, 2, -1, 1_000, 1)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(40020, "ImmediatelyMonster", Vec::new());
    let ops = crate::engine::skill::behavior::rule_ops(
        crate::engine::skill::behavior::BehaviorOpContext {
            source_uid: -1,
            source_team: 2,
            target_uid: -1,
            active_skill_id: 116362012,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    drain::run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();
    managers.hp.lose(12, 100, -1);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(100),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(200),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 2,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    for skill_id in [100, 200] {
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: Vec::new(),
        });
        catalog.insert_logic_target(skill_id, 1);
        catalog.insert_damage_rate(skill_id, 1_000);
    }

    let result = run_player_phase(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [
            RoundCommand::PlayCard {
                card_index: 0,
                target_uid: Some(-1),
                chosen_skill_id: None,
                recorded_skill: None,
            },
            RoundCommand::PlayCard {
                card_index: 0,
                target_uid: Some(-1),
                chosen_skill_id: None,
                recorded_skill: None,
            },
        ],
        Vec::new(),
        1,
        0,
    )
    .unwrap();
    let result = result.drain;

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(
        managers.entity.roster_lane(-2),
        Some(crate::engine::manager::entity::RosterLane::Main)
    );
    assert_eq!(
        managers.entity.roster_lane(11),
        Some(crate::engine::manager::entity::RosterLane::Main)
    );

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        if step.act_id == Some(200) {
            order.push("next action");
        }
        for effect in &step.act_effect {
            match effect.effect_type {
                Some(value) if value == sonettobuf::effect_type_enum::EffectType::Dead as i32 => {
                    order.push("death");
                }
                Some(value)
                    if value == sonettobuf::effect_type_enum::EffectType::Changehero as i32 =>
                {
                    order.push("promotion");
                }
                _ => {}
            }
            if let Some(child) = &effect.fight_step {
                collect(child, order);
            }
        }
    }
    let mut order = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut order);
    }
    assert_eq!(order, ["death", "promotion", "promotion", "next action"]);
}

#[test]
fn reserve_promotion_records_roster_change_and_removes_the_old_ai_cards() {
    init_config();
    let entity = |uid, hp, position| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        position: Some(position),
        ..Default::default()
    };
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 100, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 1)],
            sub_entitys: vec![entity(-2, 100, -1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(-1, 100, 10);
    let promotions = managers.promote_reserves(&mut fight);
    let pool = TargetPool::from_fight(&fight);
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

    let result = run_promotions(
        &pool,
        &mut managers,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        promotions,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();

    assert!(managers.card.ai_queue().is_empty());
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0]
            .act_effect
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Fightstep as i32,
            sonettobuf::effect_type_enum::EffectType::Changehero as i32,
        ]
    );
    assert_eq!(
        steps[0].act_effect[0]
            .fight_step
            .as_ref()
            .unwrap()
            .act_effect[0]
            .effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32)
    );
    assert_eq!(
        steps[0].act_effect[1].entity.as_ref().unwrap().uid,
        Some(-2)
    );
}

#[test]
fn promoted_defender_joins_the_normal_round_start_once() {
    init_config();
    let entity = |uid, hp, position, passive_skill| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        ex_point: Some(0),
        position: Some(position),
        passive_skill,
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 1, Vec::new()), entity(-2, 0, 2, Vec::new())],
            sub_entitys: vec![entity(-3, 100, -1, vec![40])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let promotions = managers.promote_reserves(&mut fight);
    managers.sync_roster(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
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
        skill_id: 40,
        slots: vec![slot],
    });
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        current_round: 2,
        ..Default::default()
    };

    run_promotions(
        &pool,
        &mut managers,
        &catalog,
        &mut determinism,
        context,
        promotions,
    )
    .unwrap();
    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        2,
        &[],
        None,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-3), 1);
}

#[test]
fn wave_entry_setup_runs_enter_fight_and_early_round_start() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-3),
                current_hp: Some(100),
                passive_skill: vec![2531, 2370, 2524, 2533],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_wave_entry_setup(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[-3],
    )
    .unwrap();

    fn collect_act_ids(step: &sonettobuf::FightStep, ids: &mut Vec<i32>) {
        if let Some(skill_id) = step.act_id.filter(|skill_id| *skill_id > 0) {
            ids.push(skill_id);
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            collect_act_ids(child, ids);
        }
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut act_ids = Vec::new();
    for step in &steps {
        collect_act_ids(step, &mut act_ids);
    }
    assert_eq!(steps.len(), 1);
    assert!(act_ids.contains(&2531));
    assert!(act_ids.contains(&2370));
    assert_eq!(act_ids, vec![2531, 2370]);
    assert!(!act_ids.contains(&2533));
    assert!(matches!(
        result.frames[0].owner,
        FrameOwner::RoundPhase(RoundPhase::EntityEntrySetup)
    ));
}

#[test]
fn wave_entry_round_start_condition_runs_once_before_the_first_ai_turn() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-3),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![40, 50, 60],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    for (skill_id, opcode, amount, kind) in [
        (
            40,
            727100,
            1,
            ParsedConditionKind::RoundInterval {
                start_round: 0,
                period: 1,
            },
        ),
        (50, 101, 1, ParsedConditionKind::None(NoneMode::RoundStart)),
        (60, 102, 1, ParsedConditionKind::None(NoneMode::RoundStart)),
    ] {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(20002, "AddExPoint"),
                vec![amount],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: if opcode == 727100 {
                "RoundAfter".to_owned()
            } else {
                "None".to_owned()
            },
            kind,
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: vec![slot],
        });
    }
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        current_round: 2,
        ..Default::default()
    };

    let entry = run_wave_entry_setup(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        &[-3],
    )
    .unwrap();
    managers.begin_round();
    let before_ai = run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        2,
        &[-3],
        None,
    )
    .unwrap();

    fn act_ids(result: &DrainResult) -> Vec<i32> {
        fn collect(step: &sonettobuf::FightStep, result: &mut Vec<i32>) {
            if let Some(skill_id) = step.act_id.filter(|skill_id| *skill_id > 0) {
                result.push(skill_id);
            }
            for child in step
                .act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
            {
                collect(child, result);
            }
        }
        let mut result_ids = Vec::new();
        for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
            collect(&step, &mut result_ids);
        }
        result_ids
    }
    assert_eq!(act_ids(&entry), vec![40]);
    assert_eq!(act_ids(&before_ai), vec![50, 60]);
    assert_eq!(managers.ex_point.get(-3), 3);
}

#[test]
fn wave_entry_resolves_configured_identity_before_the_next_action() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-7),
                    model_id: Some(151417),
                    team_type: Some(2),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    passive_skill: vec![1144003, 1144004],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-6),
                    model_id: Some(151415),
                    team_type: Some(2),
                    current_hp: Some(1),
                    buffs: [11430011, 11430031, 11430051]
                        .into_iter()
                        .map(|buff_id| BuffInfo {
                            buff_id: Some(buff_id),
                            duration: Some(1),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_wave_entry_setup(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        &[-7],
    )
    .unwrap();

    assert_eq!(managers.entity_snapshot(-7).unwrap().model_id, Some(151407));
}

#[test]
fn entity_entry_fans_existing_halo_to_each_entrant() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let mut source = entity(10, 1);
    source.buffs.push(BuffInfo {
        buff_id: Some(31270412),
        uid: Some(1015),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-3, -4, -5].into_iter().map(|uid| entity(uid, 2)).collect(),
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = publish_entity_entries(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[-3, -4, -5],
    )
    .unwrap();

    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Buff(changes) => Some(changes.as_ref()),
                _ => None,
            })
            .flat_map(|changes| &changes.fanout)
            .map(|fanout| {
                let added = &fanout.added[0];
                (
                    fanout.rule,
                    fanout.emitter_uid,
                    fanout.carrier_buff_uid,
                    added.target_uid,
                    added.buff.buff_id.unwrap(),
                    added.buff.uid.unwrap(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                DefinitionKey::new(771, "MasterHalo"),
                10,
                1015,
                -3,
                31270413,
                1016
            ),
            (
                DefinitionKey::new(771, "MasterHalo"),
                10,
                1015,
                -4,
                31270413,
                1017
            ),
            (
                DefinitionKey::new(771, "MasterHalo"),
                10,
                1015,
                -5,
                31270413,
                1018
            ),
        ]
    );
    assert_eq!(
        result
            .events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::BuffAdded(change) => Some(change.target_uid),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![-3, -4, -5]
    );

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 3);
    let children = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .filter_map(|effect| effect.fight_step.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3);
    assert_eq!(
        children
            .iter()
            .map(|step| (
                step.from_id.unwrap(),
                step.act_effect[0].target_id.unwrap(),
                step.act_effect[0].effect_num.unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![(10, -3, 31270413), (10, -4, 31270413), (10, -5, 31270413)]
    );
}

#[test]
fn entity_entry_orders_halo_carriers_by_owner_uid() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let mut attacker = entity(10, 1);
    attacker.buffs.push(BuffInfo {
        buff_id: Some(31270412),
        uid: Some(1015),
        from_uid: Some(10),
        ..Default::default()
    });
    let mut defender = entity(-1, 2);
    defender.buffs.push(BuffInfo {
        buff_id: Some(109320111),
        uid: Some(1016),
        from_uid: Some(-1),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![attacker],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![defender, entity(-4, 2)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = publish_entity_entries(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[-4],
    )
    .unwrap();

    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Buff(changes) => Some(changes.as_ref()),
                _ => None,
            })
            .flat_map(|changes| &changes.fanout)
            .map(|fanout| (fanout.emitter_uid, fanout.added[0].buff.buff_id.unwrap()))
            .collect::<Vec<_>>(),
        [(-1, 109320111), (10, 31270413)]
    );
}

#[test]
fn entity_entry_does_not_promote_a_derived_halo_to_a_carrier() {
    init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let mut source = entity(10);
    source.buffs.push(BuffInfo {
        buff_id: Some(109320111),
        uid: Some(1015),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source, entity(11), entity(12)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    publish_entity_entries(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[11],
    )
    .unwrap();
    let result = publish_entity_entries(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[12],
    )
    .unwrap();

    let emitters = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(changes) => Some(changes.as_ref()),
            _ => None,
        })
        .flat_map(|changes| &changes.fanout)
        .map(|fanout| fanout.emitter_uid)
        .collect::<Vec<_>>();
    assert_eq!(emitters, [10]);
}

fn entry_halo_additions(fight: &Fight, target_uids: &[i64]) -> Vec<(i64, i64)> {
    let pool = TargetPool::from_fight(fight);
    let mut managers = BattleManagers::seeded(fight);
    publish_entity_entries(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        target_uids,
    )
    .unwrap()
    .outcomes
    .into_iter()
    .filter_map(|outcome| match outcome {
        RuleOutcome::Buff(changes) => Some(changes),
        _ => None,
    })
    .flat_map(|changes| changes.fanout)
    .flat_map(|fanout| fanout.added)
    .map(|added| (added.target_uid, added.buff.uid.unwrap()))
    .collect()
}

fn publish_entity_entries(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    target_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &target_uid in target_uids {
        append(
            &mut result,
            drain::run_owner_event(
                managers,
                pool,
                catalog,
                determinism,
                context,
                BattleEvent::EntityEntered { target_uid },
                std::slice::from_ref(&target_uid),
            )?,
        );
    }
    Ok(result)
}

#[test]
fn pre_v7_entry_halo_uses_the_emitter_uid_lane() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let mut source = entity(10, 1);
    source.buffs.push(BuffInfo {
        buff_id: Some(31270412),
        uid: Some(1015),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-3, 2)],
            ..Default::default()
        }),
        version: Some(6),
        ..Default::default()
    };

    assert_eq!(entry_halo_additions(&fight, &[-3]), vec![(-3, 1016)]);
}

#[test]
fn entry_halo_filters_inactive_expired_and_duplicate_plans() {
    init_config();
    let entity = |uid, team_type, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(current_hp),
        ..Default::default()
    };
    let carrier = |buff_id, duration| BuffInfo {
        buff_id: Some(buff_id),
        uid: Some(1015),
        from_uid: Some(10),
        duration: Some(duration),
        ..Default::default()
    };

    let mut active_source = entity(10, 1, 100);
    active_source.buffs.push(carrier(31270412, 0));
    let duplicate_targets = Fight {
        attacker: Some(FightTeam {
            entitys: vec![active_source.clone()],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-3, 2, 100)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    assert_eq!(
        entry_halo_additions(&duplicate_targets, &[-3, -3]),
        vec![(-3, 1016)]
    );

    let inactive_carrier = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(11, 1, 100)],
            sub_entitys: vec![active_source.clone()],
            ..Default::default()
        }),
        defender: duplicate_targets.defender.clone(),
        version: Some(7),
        ..Default::default()
    };
    assert!(entry_halo_additions(&inactive_carrier, &[-3]).is_empty());

    let mut dead_source = active_source.clone();
    dead_source.current_hp = Some(0);
    let dead_carrier = Fight {
        attacker: Some(FightTeam {
            entitys: vec![dead_source],
            ..Default::default()
        }),
        defender: duplicate_targets.defender.clone(),
        version: Some(7),
        ..Default::default()
    };
    assert!(entry_halo_additions(&dead_carrier, &[-3]).is_empty());

    let inactive_entrant = Fight {
        attacker: duplicate_targets.attacker.clone(),
        defender: Some(FightTeam {
            entitys: vec![entity(-4, 2, 100)],
            sub_entitys: vec![entity(-3, 2, 100)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    assert!(entry_halo_additions(&inactive_entrant, &[-3]).is_empty());

    let mut expired_source = entity(10, 1, 100);
    expired_source.buffs.push(carrier(30860151, 0));
    let expired_carrier = Fight {
        attacker: Some(FightTeam {
            entitys: vec![expired_source, entity(12, 1, 100)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    assert!(entry_halo_additions(&expired_carrier, &[12]).is_empty());
}
