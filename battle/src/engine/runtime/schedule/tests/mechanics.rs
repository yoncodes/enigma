use super::*;

#[test]
fn card_energy_allocation_commits_cards_then_spends_only_allocated_gauge() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2240000),
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
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: (0..3)
                .map(|index| CardInfo {
                    uid: Some(10),
                    skill_id: Some(200 + index),
                    card_effect: Some(1),
                    energy: Some(0),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 30,
        }))
        .unwrap();
    let features = managers.buff.active_features(&managers.hp);
    let enabled = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &features,
        99998,
    )
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter else {
        unreachable!()
    };
    managers.execute_emitter(command);
    for op in [enabled.team_energy, enabled.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            unreachable!()
        };
        managers.execute_gauge(command).unwrap();
    }
    let tag = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::EmitterTag))
        .unwrap();
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            buff_act::feature_command_origin(tag).unwrap(),
            team_energy_key(1),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 3 },
        ))
        .unwrap();

    let result = run_card_energy_allocation(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();

    assert!(matches!(
        result.outcomes.as_slice(),
        [RuleOutcome::Gauge(gauge), RuleOutcome::Card(cards)]
            if cards.kind
                == crate::engine::manager::card::CardChangeKind::EnergyAllocated
                && gauge.applied_delta == -3
    ));
    assert_eq!(managers.card.hand()[2].energy, Some(3));
    assert_eq!(managers.gauge.get(team_energy_key(1)).unwrap().current, 0);
}

#[test]
fn compiled_team_tag_setup_enables_impromptu_and_single_owner_bloodtithe_reactions() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(20_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(6_270_501),
                        from_uid: Some(10),
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

    crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        crate::engine::skill::rule::SetupStage::BattleStart,
        0,
    )
    .unwrap();

    assert!(managers.gauge.get(team_energy_key(1)).is_some());
    assert!(managers.gauge.get(inspiration_key(99_998)).is_some());
    let blood_key = crate::engine::mechanic::bloodtithe::rule::key(1);
    assert!(managers.gauge.get(blood_key).is_some());

    crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Lose(crate::engine::manager::hp::HpLoss {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(30006, "LostLife"),
                },
                source_uid: 10,
                target_uid: 10,
                amount: 9_070,
                config_effect: 30006,
                hurt: None,
            }),
        ))],
    )
    .unwrap();

    assert!(managers.gauge.get(blood_key).unwrap().current > 0);
}

#[test]
fn enter_fight_add_buff_materializes_configured_shields() {
    init_config();
    let entity = |uid, model_id, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(10_000),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3127, vec![31270148]),
                entity(11, 3139, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::EnterFight,
        0,
    )
    .unwrap();

    assert_eq!(managers.hp.shield(10), 1_800);
    assert_eq!(managers.hp.shield(11), 1_800);
    assert!(managers.buff.has_buff_id(10, 31270502));
    assert!(managers.buff.has_buff_id(11, 31270502));
}

#[test]
fn lorentz_passive_uses_team_tags_for_moxie_and_snapshots_lingering_glow_bonuses() {
    init_config();
    let entity = |uid, model_id, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(10_000),
        ex_point: Some(0),
        ex_skill: Some(if model_id == 3139 { 31390131 } else { 31340131 }),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3139, vec![31390173]),
                entity(11, 3134, Vec::new()),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1000),
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
    let catalog = crate::engine::skill::effect::catalog::global();
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext::default();

    for stage in [SetupStage::BattleStart, SetupStage::EnterFight] {
        crate::engine::runtime::drain::run_setup_stage(
            &mut managers,
            &pool,
            catalog,
            &mut determinism,
            context,
            stage,
            0,
        )
        .unwrap();
    }
    assert_eq!(managers.ex_point.get(10), 2);
    managers.buff.add(&managers.hp, 10, -1, 4150001, 200);
    assert_eq!(managers.buff.buff_id_amount(-1, 4150001), 35);

    let gauge_origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestLingeringGlow"),
    };
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            gauge_origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            gauge_origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 250 },
        ))
        .unwrap();

    for &(stage, priority) in &[
        (SetupStage::RoundStartCondition, 100),
        (SetupStage::RoundStartCondition, 101),
        (SetupStage::RoundStartCondition, 102),
        (SetupStage::RoundStart, 1),
    ] {
        crate::engine::runtime::drain::run_setup_stage(
            &mut managers,
            &pool,
            catalog,
            &mut determinism,
            context,
            stage,
            priority,
        )
        .unwrap();
    }

    for uid in [10, 11] {
        assert!(managers.buff.has_buff_id(uid, 31390190));
        assert!(managers.buff.has_buff_id(uid, 31390152));
        for (buff_id, expected) in [(313901734, 275), (313901744, 150)] {
            let buff = managers
                .buff
                .active_for(uid)
                .find(|buff| buff.buff_id == Some(buff_id))
                .unwrap();
            assert_eq!(buff.act_info[0].act_id, Some(1053));
            assert_eq!(buff.act_info[0].param, vec![expected]);
        }
    }

    run_attacker_round_end(&mut managers, &pool, catalog, &mut determinism, context).unwrap();

    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(managers.ex_point.get(11), 1);
}

#[test]
fn three_lingering_glow_allies_replace_the_five_burn_cap_bonus_with_twenty() {
    init_config();
    let entity = |uid, model_id, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(100),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3139, vec![31390173]),
                entity(11, 3134, Vec::new()),
                FightEntityInfo {
                    destiny_stone: Some(308101),
                    destiny_rank: Some(4),
                    ..entity(12, 3081, Vec::new())
                },
            ],
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

    for stage in [SetupStage::BattleStart, SetupStage::EnterFight] {
        crate::engine::runtime::drain::run_setup_stage(
            &mut managers,
            &pool,
            crate::engine::skill::effect::catalog::global(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            stage,
            0,
        )
        .unwrap();
    }

    assert!(!managers.buff.has_buff_id(10, 31390161));
    assert!(managers.buff.has_buff_id(10, 31390163));
    managers.buff.add(&managers.hp, 10, -1, 4150001, 100);
    assert_eq!(managers.buff.buff_id_amount(-1, 4150001), 50);
}

#[test]
fn action_queue_commit_collects_played_card_energy_as_inspiration() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2240000),
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
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(200),
                energy: Some(3),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 30,
        }))
        .unwrap();
    let features = managers.buff.active_features(&managers.hp);
    let tag = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::EmitterTag))
        .unwrap();
    let enabled = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &features,
        99998,
    )
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter else {
        unreachable!()
    };
    managers.execute_emitter(command);
    for op in [enabled.team_energy, enabled.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            unreachable!()
        };
        managers.execute_gauge(command).unwrap();
    }
    managers
        .execute_card(CardCommand::Play(CardPlay {
            origin: buff_act::feature_command_origin(tag).unwrap(),
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    let result = run_action_queue_committed(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
        99998,
        0,
    )
    .unwrap();

    assert!(matches!(
        result.outcomes.as_slice(),
        [
            RuleOutcome::Card(cards),
            RuleOutcome::Gauge(change),
            RuleOutcome::Card(queued)
        ]
            if cards.kind == crate::engine::manager::card::CardChangeKind::ActionQueueCommitted
                && change.applied_delta == 3
                && queued.kind == crate::engine::manager::card::CardChangeKind::UseCardQueued
    ));
    assert_eq!(
        result
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![EventKind::ActionQueueCommitted, EventKind::GaugeChanged]
    );
    assert_eq!(
        managers.gauge.get(inspiration_key(99998)).unwrap().current,
        3
    );
    assert_eq!(managers.card.queued_use_cards().len(), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].act_effect.len(), 3);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Usecards as i32)
    );
    assert_eq!(
        steps[0].act_effect[2].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
    );
    assert_eq!(
        steps[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Emitterenergychange as i32)
    );
    assert_eq!(steps[1].from_id, Some(0));
    assert_eq!(
        steps[2].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Addusecard as i32)
    );
    assert_eq!(steps[2].from_id, Some(0));
}

#[test]
fn player_actions_resolved_activates_internal_inspiration_thresholds() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2240000),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(99998),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31080151),
                        from_uid: Some(99998),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let features = managers.buff.active_features(&managers.hp);
    let tag = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::EmitterTag))
        .unwrap();
    let enabled = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &features,
        99998,
    )
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter else {
        unreachable!()
    };
    managers.execute_emitter(command);
    for op in [enabled.team_energy, enabled.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            unreachable!()
        };
        managers.execute_gauge(command).unwrap();
    }
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            buff_act::feature_command_origin(tag).unwrap(),
            inspiration_key(99998),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 6 },
        ))
        .unwrap();

    let activated = run_player_actions_resolved(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
        99998,
    )
    .unwrap();

    assert_eq!(activated.outcomes.len(), 2);
    assert_eq!(
        activated
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![EventKind::PlayerActionsResolved]
    );
    let steps = crate::engine::packet::timeline::project(&activated.frames).unwrap();
    assert!(steps.is_empty());
    assert!(managers.buff.has_buff_id(99998, 31080152));
    assert!(managers.buff.has_buff_id(99998, 31080153));
    let plan = build_plan(&managers, 1, 99998).unwrap();
    let definition =
        crate::catalog::impromptu_definition(crate::test_support::game_data()).unwrap();
    assert_eq!(plan.skill_id, definition.skill_id());
    assert_eq!(plan.inspiration, 6);
    assert_eq!(plan.attack_count, 3);
    assert_eq!(plan.damage_rate, definition.damage_rate(6));

    let resolved = run_impromptu_resolved(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        ImpromptuResolution {
            team: 1,
            emitter_uid: 99998,
            critical_action_count: 0,
        },
    )
    .unwrap();

    assert_eq!(
        resolved
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![EventKind::ImpromptuResolved, EventKind::GaugeChanged]
    );
    assert_eq!(
        managers.gauge.get(inspiration_key(99998)).unwrap().current,
        0
    );
}

#[test]
fn idle_impromptu_finalizes_card_and_emitter_energy_once() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2_240_000),
                    from_uid: Some(10),
                    ..Default::default()
                }],
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
    let features = managers.buff.active_features(&managers.hp);
    let enabled = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &features,
        99_998,
    )
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter else {
        unreachable!()
    };
    managers.execute_emitter(command);
    for op in [enabled.team_energy, enabled.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            unreachable!()
        };
        managers.execute_gauge(command).unwrap();
    }

    let result = run_impromptu(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
        99_998,
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(matches!(
        steps.as_slice(),
        [finalization]
            if finalization.act_effect.len() == 2
                && finalization.act_effect[0].effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Allocatecardenergy as i32)
                && finalization.act_effect[0].effect_num1 == Some(0)
                && finalization.act_effect[1].effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Emitterenergychange as i32)
                && finalization.act_effect[1].effect_num1 == Some(0)
    ));
}

#[test]
fn impromptu_executes_the_transient_plan_then_resets_inspiration() {
    init_config();
    let fighter = |uid, hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(1000),
            defense: Some(100),
            mdefense: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut owner = fighter(10, 10_000);
    owner.buffs.push(BuffInfo {
        uid: Some(20),
        buff_id: Some(2240000),
        from_uid: Some(10),
        ..Default::default()
    });
    let mut emitter = fighter(99998, 10_000);
    emitter.buffs.push(BuffInfo {
        uid: Some(30),
        buff_id: Some(31080151),
        from_uid: Some(99998),
        ..Default::default()
    });
    emitter.buffs.push(BuffInfo {
        uid: Some(31),
        buff_id: Some(30480241),
        from_uid: Some(99998),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![owner, emitter],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![fighter(-1, 100_000), fighter(-2, 100_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let features = managers.buff.active_features(&managers.hp);
    let tag = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::EmitterTag))
        .unwrap();
    let enabled = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &features,
        99998,
    )
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter else {
        unreachable!()
    };
    managers.execute_emitter(command);
    for op in [enabled.team_energy, enabled.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            unreachable!()
        };
        managers.execute_gauge(command).unwrap();
    }
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            buff_act::feature_command_origin(tag).unwrap(),
            inspiration_key(99998),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 6 },
        ))
        .unwrap();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let definition =
        crate::catalog::impromptu_definition(crate::test_support::game_data()).unwrap();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(definition.skill_id(), 99998, [true; 6]);

    let result = run_impromptu(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        1,
        99998,
    )
    .unwrap();

    assert!(managers.hp.current(-1) < 100_000);
    assert!(managers.hp.current(-2) < 100_000);
    assert_eq!(
        managers.gauge.get(inspiration_key(99998)).unwrap().current,
        0
    );
    assert!(
        result
            .events
            .iter()
            .any(|event| event.kind() == EventKind::ImpromptuResolved)
    );
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::ImpromptuResolved {
            critical_action_count: 3,
            ..
        }
    )));
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Emitterfightnotify as i32)
        })
    }));
}
