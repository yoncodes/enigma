use super::*;

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
            EventKind::RoundEndAfterOwnerSettlement,
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
            EventKind::RoundEndEntitySettlement,
            EventKind::RoundEndAfterSettlement,
            EventKind::RoundEndAfterOwnerSettlement,
            EventKind::RoundEndFinalSettlement,
            EventKind::RoundEndAfterSideFinalExpiry,
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
fn defeated_conduit_devices_refresh_only_after_terminal_commitment() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3134),
                    current_hp: Some(100),
                    ..Default::default()
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
    managers.hp.lose(10, 100, -1);
    let mut determinism = RoundDeterminism::default();
    let catalog = SkillEffectCatalog::default();

    let active = run_terminal_conduit_roster_refresh(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
    )
    .unwrap();
    assert!(active.frames.is_empty());
    assert_eq!(managers.conduit.selected_group(10), Some(1));

    assert!(managers.commit_terminal(crate::engine::round::outcome::BattleOutcome::Defeat));
    let terminal = run_terminal_conduit_roster_refresh(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
    )
    .unwrap();
    assert_eq!(managers.conduit.selected_group(10), None);
    let steps = crate::engine::packet::timeline::project(&terminal.frames).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Initdevice as i32)
    );
}

#[test]
fn final_settlement_runs_side_expiry_reactions_before_entity_expiry() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                ex_point_type: Some(4),
                ex_point_max: Some(100),
                passive_skill: vec![302501432],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(435621),
                        from_uid: Some(10),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(8011),
                        from_uid: Some(10),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(109320112),
                        duration: Some(1),
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .buff
        .execute(
            &managers.hp,
            crate::engine::manager::buff::BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(1, "AddBuff"),
                    },
                    source_uid: -1,
                    target_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
                    buff_id: 109320001,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ),
        )
        .unwrap();
    managers
        .execute_buff(
            crate::engine::manager::buff::BuffCommand::AccumulateActValue(
                crate::engine::manager::buff::BuffAccumulateActValue {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(1019, "LostHpCountAddBuff"),
                    },
                    target_uid: 10,
                    buff_uid: 20,
                    act_id: 1019,
                    delta: 1,
                },
            ),
        )
        .unwrap();

    let result = run_round_end_final_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();
    fn has_output(frame: &crate::engine::runtime::record::SemanticFrame) -> bool {
        frame.items.iter().any(|item| match item {
            FrameItem::Change(_) | FrameItem::Cue(_) => true,
            FrameItem::Child(child) => has_output(child),
        })
    }
    assert_eq!(result.frames.len(), 2);
    assert_eq!(
        result.frames[0].owner,
        FrameOwner::RoundPhase(RoundPhase::RoundEndFinalSettlement)
    );
    assert!(has_output(&result.frames[0]));
    assert_eq!(
        result.frames[1].owner,
        FrameOwner::RoundPhase(RoundPhase::RoundEndAfterFinalSettlement)
    );
    assert!(!has_output(&result.frames[1]));
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();

    fn contains_effect(step: &sonettobuf::FightStep, effect_type: i32, target_uid: i64) -> bool {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(effect_type) && effect.target_id == Some(target_uid)
                || effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|child| contains_effect(child, effect_type, target_uid))
        })
    }

    fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
        step.act_id == Some(skill_id)
            || step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|child| contains_skill(child, skill_id))
            })
    }

    assert_eq!(steps.len(), 1);
    assert!(contains_effect(
        &steps[0],
        sonettobuf::effect_type_enum::EffectType::Buffadd as i32,
        10,
    ));
    assert!(contains_effect(&steps[0], 111, 10));
    assert!(!contains_skill(&steps[0], 302501432));
    assert_eq!(managers.ex_point.get(10), 15);
    assert!(managers.buff.snapshot(10, 22).is_none());
    assert!(
        !managers
            .buff
            .has_active_buff_id(crate::engine::fight::rules::ATTACKER_SIDE_UID, 109320001)
    );

    fn effect_types(step: &sonettobuf::FightStep, output: &mut Vec<i32>) {
        for effect in &step.act_effect {
            if let Some(effect_type) = effect.effect_type {
                output.push(effect_type);
            }
            if let Some(child) = effect.fight_step.as_ref() {
                effect_types(child, output);
            }
        }
    }
    let mut effects = Vec::new();
    effect_types(&steps[0], &mut effects);
    let expired = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            (*effect == sonettobuf::effect_type_enum::EffectType::Buffdel as i32).then_some(index)
        })
        .collect::<Vec<_>>();
    let resource = effects.iter().position(|effect| *effect == 111).unwrap();
    assert_eq!(expired.len(), 2);
    assert!(expired[0] < resource && resource < expired[1]);
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
fn defender_entity_settlement_nests_its_periodic_passive() {
    init_config();
    let fight = Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                passive_skill: vec![109380006],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[-1],
        SettlementSide::Defender,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.output.frames).unwrap();

    fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
        step.act_id == Some(skill_id)
            || step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|nested| contains_skill(nested, skill_id))
            })
    }

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].act_id, Some(0));
    assert!(contains_skill(&steps[0], 109380006));
    assert!(managers.buff.has_buff_id(-1, 109320106));
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
