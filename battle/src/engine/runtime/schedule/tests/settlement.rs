use super::*;

#[test]
fn round_end_settlement_drains_dot_layers_then_durations() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31340001),
                        layer: Some(4),
                        count: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(30),
                        buff_id: Some(530000414),
                        duration: Some(2),
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

    let result = run_round_end_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10],
    )
    .unwrap();

    assert_eq!(result.outcomes.len(), 2);
    assert_eq!(managers.buff.snapshot(10, 20).unwrap().layer, Some(2));
    assert_eq!(managers.buff.snapshot(10, 30).unwrap().duration, Some(1));
    assert_eq!(
        crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn entity_settlement_uses_attacker_and_defender_roster_order() {
    init_config();
    let entity = |uid, buff_uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        power_infos: vec![PowerInfo {
            power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
            num: Some(2),
            max: Some(5),
        }],
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(530000414),
            duration: Some(2),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 20), entity(11, 21)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, -20)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.eureka.add(
        10,
        10,
        crate::engine::manager::eureka::EUREKA_RESOURCE_ID,
        -1,
        0,
    );

    let attacker = run_entity_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10, 11],
        SettlementSide::Attacker,
    )
    .unwrap();

    assert_eq!(
        attacker
            .output
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            EventKind::RoundEndEntitySettlement,
            EventKind::RoundEndAfterSettlement,
        ]
    );
    assert_eq!(managers.buff.snapshot(10, 20).unwrap().duration, Some(1));
    assert_eq!(managers.buff.snapshot(11, 21).unwrap().duration, Some(1));
    assert_eq!(
        managers
            .eureka
            .round_change(10, crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
        Default::default()
    );

    let defender = run_entity_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[-1],
        SettlementSide::Defender,
    )
    .unwrap();

    assert_eq!(
        defender
            .output
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            EventKind::RoundEndEntitySettlement,
            EventKind::RoundEndAfterSettlement,
        ]
    );
    assert!(defender.settled_buffs.is_empty());
    assert_eq!(managers.buff.snapshot(-1, -20).unwrap().duration, Some(1));
}

#[test]
fn entity_settlement_buff_presence_fires_once_only_for_the_matching_owner() {
    init_config();
    let entity = |uid, buffs| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: vec![100],
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31460001),
                        from_uid: Some(10),
                        layer: Some(1),
                        ..Default::default()
                    }],
                ),
                entity(11, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = crate::engine::skill::condition::parse::parse_conditions(
        config::configs::get(),
        "19303#31460001",
    );
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });

    let subscribers = crate::engine::skill::subscriber::for_compiled_owner_events(
        &pool,
        &managers,
        &catalog,
        [EventKind::RoundEndEntitySettlement],
        &[10, 11],
    )
    .unwrap();
    assert_eq!(
        subscribers
            .skills
            .iter()
            .map(|subscriber| subscriber.owner_uid)
            .collect::<Vec<_>>(),
        vec![10, 11]
    );

    run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10, 11],
        SettlementSide::Attacker,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(11), 0);
}

#[test]
fn entity_settlement_keeps_event_owned_buff_changes_outside_the_nested_skill() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                model_id: Some(3146),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![100],
                buffs: vec![
                    BuffInfo {
                        uid: Some(1006),
                        buff_id: Some(31460143),
                        from_uid: Some(10),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1139),
                            param: vec![70_000],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1150),
                        buff_id: Some(31460001),
                        from_uid: Some(10),
                        layer: Some(2),
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
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60305, "ConsumeBuffMeiLeiEr"),
        Vec::new(),
        ["31460001", "1", "8000", "1", "31460111,1"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    );
    let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
    slot.conditions = crate::engine::skill::condition::parse::parse_conditions(
        config::configs::get(),
        "19303#31460001",
    );
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });

    let result = run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10],
        SettlementSide::Attacker,
    )
    .unwrap();

    assert_eq!(managers.buff.snapshot(10, 1150).unwrap().layer, Some(1));
    assert_eq!(
        managers.buff.snapshot(10, 1006).unwrap().act_info[0].param,
        vec![78_000]
    );
    assert!(managers.buff.has_buff_id(10, 31460111));
    assert_eq!(managers.ex_point.get(10), 1);

    fn find_parent(
        step: &sonettobuf::FightStep,
        skill_id: i32,
    ) -> Option<(&sonettobuf::FightStep, &sonettobuf::FightStep)> {
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            if child.act_id == Some(skill_id) {
                return Some((step, child));
            }
            if let Some(found) = find_parent(child, skill_id) {
                return Some(found);
            }
        }
        None
    }
    let steps = crate::engine::packet::timeline::project(&result.output.frames).unwrap();
    let (parent, skill) = steps
        .iter()
        .find_map(|step| find_parent(step, 100))
        .expect("entity settlement must contain the passive skill");
    let buff_index = |buff_id| {
        parent
            .act_effect
            .iter()
            .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(buff_id))
    };
    let skill_index = parent
        .act_effect
        .iter()
        .position(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_id == Some(100))
        })
        .unwrap();
    let consume_index = buff_index(31460001).expect("parent owns the consumed buff update");
    let reward_index = buff_index(31460111).expect("parent owns the reward buff grant");
    assert!(skill_index < consume_index && consume_index < reward_index);
    assert!(skill.act_effect.iter().all(|effect| {
        !matches!(
            effect.buff.as_ref().and_then(|buff| buff.buff_id),
            Some(31460001 | 31460111)
        )
    }));
    assert!(skill.act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
            && effect
                .buff_act_info
                .as_ref()
                .is_some_and(|info| info.act_id == Some(1139) && info.param == [78_000])
    }));
    assert!(skill.act_effect.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    }));
}

#[cfg(feature = "private-fixtures")]
#[test]
fn setup_reserves_summoned_lanes_before_the_next_buff() {
    init_config();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../battle_preview/fixtures/battles/battle7/StartDungeonReply.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    sonettobuf::normalize::normalize_live_json(&mut value);
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    assert!(managers.summon.get(232267340, 150011).is_some());
    assert!(managers.summon.get(232267340, 150021).is_some());
    let next = managers
        .buff
        .add(&managers.hp, 240494289, 240494289, 31070111, 0)
        .unwrap();
    assert_eq!(next.buff.uid, Some(55));
}

#[cfg(feature = "private-fixtures")]
#[test]
fn heat_tag_setup_selects_lingering_glow_over_bloodtithe() {
    init_config();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../battle_preview/fixtures/battles/battle6/StartDungeonReply.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    sonettobuf::normalize::normalize_live_json(&mut value);
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    assert!(
        managers
            .gauge
            .get(crate::engine::mechanic::lingering_glow::key(1))
            .is_none()
    );

    let result = crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        crate::engine::skill::rule::SetupStage::BattleStart,
        0,
    )
    .unwrap();

    assert!(
        managers
            .gauge
            .get(crate::engine::mechanic::lingering_glow::key(1))
            .is_some()
    );
    assert!(
        managers
            .gauge
            .get(crate::engine::mechanic::bloodtithe::rule::key(1))
            .is_none()
    );
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Gauge(change)
            if change.key == crate::engine::mechanic::lingering_glow::key(1)
    )));
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(
        steps
            .iter()
            .flat_map(|step| &step.act_effect)
            .filter_map(|effect| {
                matches!(
                    effect.effect_type,
                    Some(value)
                        if value
                            == sonettobuf::effect_type_enum::EffectType::Emittercreate as i32
                            || value
                                == sonettobuf::effect_type_enum::EffectType::Bloodpoolmaxcreate
                                    as i32
                )
                .then_some(effect.effect_type.unwrap())
            })
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Emittercreate as i32,
            sonettobuf::effect_type_enum::EffectType::Bloodpoolmaxcreate as i32,
        ]
    );
}

#[test]
fn no_action_round_reads_each_owners_committed_card_history() {
    init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: vec![100],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(20)],
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
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::Play(CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 46301,
        type_name: "NoActRound".to_owned(),
        kind: ParsedConditionKind::NoActionRound,
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });

    run_no_action_round(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10, 20],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.ex_point.get(20), 1);
}

#[test]
fn side_round_end_schedules_keep_the_proven_lane_order() {
    init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
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
    let catalog = SkillEffectCatalog::default();
    let mut determinism = RoundDeterminism::default();

    let attacker = run_attacker_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
    )
    .unwrap();
    assert_eq!(
        attacker
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            EventKind::NoActionRound,
            EventKind::SmallRoundEnd,
            EventKind::RoundEnd,
            EventKind::RoundEnd,
            EventKind::RoundEndEntitySettlement,
            EventKind::RoundEndAfterSettlement,
        ]
    );

    let after_ai = run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
    )
    .unwrap();
    assert_eq!(
        after_ai
            .events
            .iter()
            .map(BattleEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            EventKind::RoundEnd,
            EventKind::SmallRoundEnd,
            EventKind::RoundEnd,
            EventKind::RoundEndEntitySettlement,
            EventKind::RoundEndAfterSettlement,
            EventKind::RoundEndFinalSettlement,
        ]
    );
    let entity_settlement = after_ai
        .frames
        .iter()
        .position(|frame| {
            matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::EntitySettlement)
            )
        })
        .unwrap();
    let transition_cue = after_ai
        .frames
        .iter()
        .position(|frame| {
            matches!(
                frame.items.as_slice(),
                [FrameItem::Cue(RoundCue::SmallRoundEnd { team_type: 1 })]
            )
        })
        .unwrap();
    assert!(entity_settlement < transition_cue);

    let steps = crate::engine::packet::timeline::project(&after_ai.frames).unwrap();
    let transition = steps
        .iter()
        .rev()
        .take(2)
        .map(|step| step.act_effect[0].effect_type.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        transition,
        vec![
            sonettobuf::effect_type_enum::EffectType::Clearuniversalcard as i32,
            sonettobuf::effect_type_enum::EffectType::Smallroundend as i32,
        ]
    );
}

#[test]
fn scalding_incantations_settle_current_hp_in_one_round_end_frame() {
    init_config();
    let entity = |uid, hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1_000), entity(11, 800)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |owner_uid| CardInfo {
        uid: Some(owner_uid),
        enchants: vec![sonettobuf::CardEnchant {
            enchant_id: Some(crate::engine::manager::card::EnchantedType::Burn.id()),
            duration: Some(-1),
            ..Default::default()
        }],
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.card =
        crate::engine::manager::card::CardManager::new(vec![card(10), card(11), card(10)]);

    let result = run_attacker_round_end(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 800);
    assert_eq!(managers.hp.current(11), 720);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let scalding = steps
        .iter()
        .find(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Enchantburndamage as i32)
            })
        })
        .unwrap();
    assert_eq!(scalding.act_effect.len(), 2);
    assert_eq!(
        scalding
            .act_effect
            .iter()
            .map(|effect| {
                (
                    effect.target_id,
                    effect.effect_num,
                    effect
                        .hurt_info
                        .as_ref()
                        .and_then(|hurt| hurt.damage_from_type),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some(10), Some(200), Some(0)),
            (Some(11), Some(80), Some(0))
        ]
    );
}

#[test]
fn attacker_round_end_includes_the_assist_boss_passive_owner() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(50),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                passive_skill: vec![12_410_022],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_attacker_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 55);
    fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
        step.act_id == Some(skill_id)
            || step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|nested| contains_skill(nested, skill_id))
            })
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(|step| contains_skill(step, 12_410_022)));
}

#[test]
fn defender_after_settlement_runs_its_registered_passive_skill() {
    init_config();
    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            defense: Some(100),
            mdefense: Some(100),
            ..Default::default()
        }),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-3, 2, vec![22_302_342])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let subscribers = crate::engine::skill::subscriber::for_compiled_owner_events(
        &pool,
        &managers,
        &catalog,
        [EventKind::RoundEndAfterSettlement],
        &[-3],
    )
    .unwrap();
    assert!(
        subscribers
            .skills
            .iter()
            .any(|subscriber| { subscriber.owner_uid == -3 && subscriber.skill_id == 22_302_342 })
    );
    let result = run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
    )
    .unwrap();

    fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
        step.act_id == Some(skill_id)
            || step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|nested| contains_skill(nested, skill_id))
            })
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(
        steps.iter().any(|step| contains_skill(step, 22_302_342)),
        "frames={:#?}",
        result.frames
    );
    assert!(steps.iter().any(|step| contains_skill(step, 22_302_351)));
}

#[test]
fn special_count_channel_casts_once_then_deletes_its_carrier() {
    init_config();
    let entity = |uid, team_type, model_id, buffs| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        model_id: Some(model_id),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            defense: Some(100),
            mdefense: Some(100),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(
                10,
                1,
                3107,
                vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31070131),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
            )],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1000, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(31070111),
            status: Some(0),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(31070121),
            status: Some(0),
            ..Default::default()
        },
    ]);
    let mut determinism = RoundDeterminism::default();
    let subscribers = crate::engine::skill::subscriber::for_compiled_owner_events(
        &pool,
        &managers,
        &catalog,
        [EventKind::RoundEndEntitySettlement],
        &[10],
    )
    .unwrap();
    assert!(subscribers.buff_acts.iter().any(|subscriber| {
        subscriber.buff_uid == 20
            && subscriber
                .key
                .definition
                .matches(1002, "SpecialCountCastChannel")
    }));

    let first = run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        &[10],
        SettlementSide::Attacker,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(10, 31070131));
    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect.iter().find_map(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .and_then(|nested| find_step(nested, act_id))
            })
        })
    }
    fn collect_steps<'a>(
        step: &'a sonettobuf::FightStep,
        act_id: i32,
        found: &mut Vec<&'a sonettobuf::FightStep>,
    ) {
        if step.act_id == Some(act_id) {
            found.push(step);
        }
        for nested in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            collect_steps(nested, act_id, found);
        }
    }
    let steps = crate::engine::packet::timeline::project(&first.output.frames).unwrap();
    let mut channel = Vec::new();
    for step in &steps {
        collect_steps(step, 31070131, &mut channel);
    }
    assert_eq!(channel.len(), 2, "steps={steps:#?}");
    assert!(find_step(channel[0], 31070151).is_some());
    let removed = &channel[1].act_effect[0];
    assert_eq!(
        removed.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
    );
    assert_eq!(removed.buff.as_ref().and_then(|buff| buff.uid), Some(20));

    let (_, next_round) = run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
    )
    .unwrap();
    let next_round = crate::engine::packet::timeline::project(&next_round.frames).unwrap();
    let cards = next_round
        .iter()
        .flat_map(|step| &step.act_effect)
        .flat_map(|effect| &effect.card_info_list)
        .filter(|card| card.uid == Some(10))
        .collect::<Vec<_>>();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().all(|card| card.status == Some(0)));

    let second = run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[10],
        SettlementSide::Attacker,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&second.output.frames).unwrap();
    assert!(!steps.iter().any(|step| find_step(step, 31070151).is_some()));
}
