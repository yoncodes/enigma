use sonettobuf::{BuffInfo, FightEntityInfo, FightHurtInfo, FightTeam};

use super::*;

fn card(uid: i64, skill_id: i32, temporary: bool) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temporary),
        ..Default::default()
    }
}

#[test]
fn opening_ultimate_reports_the_rejected_fight_task_parameter() {
    crate::init_test_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_skill: Some(20),
                ..Default::default()
            }],
            ..Default::default()
        }),
        fight_task_box: Some(sonettobuf::FightTaskBox {
            tasks: vec![sonettobuf::FightTask {
                task_id: Some(381207),
                ..Default::default()
            }],
        }),
        ..Default::default()
    };
    let round = FightRound {
        team_a_cards1: vec![card(10, 20, false)],
        ..Default::default()
    };

    let error = opening_determinism(config::configs::get(), &fight, &round).unwrap_err();
    assert_eq!(
        error,
        "prepare opening hand for replay setup: unsupported fight task 381207: \
         taskParam1=\"Round#8\": unsupported fight task kind 'Round'"
    );
}

#[test]
fn card_level_add_choice_reads_the_captured_hand_position() {
    use sonettobuf::effect_type_enum::EffectType;
    let add = |buff_act_id| sonettobuf::ActEffect {
        effect_type: Some(EffectType::Cardleveladd as i32),
        buff_act_id,
        ..Default::default()
    };
    let change = |target_id| sonettobuf::ActEffect {
        effect_type: Some(EffectType::Cardlevelchange as i32),
        target_id: Some(target_id),
        entity: Some(FightEntityInfo {
            uid: Some(10),
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_effect: vec![
                add(None),
                change(4),
                add(Some(701)),
                change(7),
                add(Some(701)),
                change(1),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let choice = |hand_index| HandRankChoice {
        opcode: 701,
        owner_uid: 10,
        hand_index,
    };

    assert_eq!(card_level_add_choices(&round), vec![choice(6), choice(0)]);
}

#[test]
fn an_ungrouped_resource_delta_is_not_read_as_a_conduit_roll_pass() {
    crate::init_test_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        passive_skill: vec![103377010],
        ex_point: Some(0),
        ex_point_max: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        ex_point_info: [10, 11]
            .into_iter()
            .map(|uid| sonettobuf::FightExPointInfo {
                uid: Some(uid),
                ex_point_type: Some(4),
                ..Default::default()
            })
            .collect(),
        fight_step: vec![FightStep {
            act_effect: vec![sonettobuf::ActEffect {
                target_id: Some(10),
                effect_type: Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32),
                effect_num: Some(50),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut replay = opening_determinism(config::configs::get(), &fight, &round).unwrap();
    for source_uid in [10, 11] {
        assert_eq!(
            replay.condition_random_roll(103377010, source_uid, 1022101),
            999
        );
    }
}

#[test]
fn observed_skill_effect_targets_keep_their_first_seen_order() {
    use sonettobuf::{ActEffect, fight_hurt_info::DamageFromType};

    let effect = |target_id, damage_from_type| ActEffect {
        target_id: Some(target_id),
        hurt_info: Some(FightHurtInfo {
            damage_from_type: Some(damage_from_type as i32),
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_id: Some(109380001),
            from_id: Some(-1),
            act_effect: vec![
                effect(40, DamageFromType::SkillEffect),
                effect(20, DamageFromType::SkillEffect),
                effect(30, DamageFromType::SkillEffect),
                effect(10, DamageFromType::SkillEffect),
                effect(40, DamageFromType::SkillEffect),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_logic_target(109380001, 202);
    let choices = skill_target_choices(&catalog, &round);

    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].skill_id, 109380001);
    assert_eq!(choices[0].target_code, 202);
    assert_eq!(choices[0].additional_targets, vec![40, 20, 30, 10]);
}

#[test]
fn moxie_reduction_import_keeps_resisted_and_applied_outcomes() {
    use battle::tooling::scan::{
        ParsedBehavior, ParsedCondition, ParsedConditionKind, ParsedSkillEffect, TargetRequest,
        behavior::classify::BehaviorSpec,
    };
    use sonettobuf::{ActEffect, effect_type_enum::EffectType, fight_hurt_info::DamageFromType};

    let mut catalog = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(30001, "DelExPoint"),
            vec![1],
            vec!["1".into()],
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 30208,
        type_name: "AttackCrit".into(),
        kind: ParsedConditionKind::AttackCrit,
        raw_args: Vec::new(),
    }];
    catalog.insert(ParsedSkillEffect {
        skill_id: 302524112,
        slots: vec![slot],
    });
    let action = |result: ActEffect| FightStep {
        act_id: Some(302524112),
        from_id: Some(10),
        act_effect: vec![
            ActEffect {
                target_id: Some(-1),
                effect_type: Some(EffectType::Crit as i32),
                hurt_info: Some(FightHurtInfo {
                    damage_from_type: Some(DamageFromType::Skill as i32),
                    ..Default::default()
                }),
                ..Default::default()
            },
            result,
        ],
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![
            action(ActEffect {
                target_id: Some(-1),
                effect_type: Some(EffectType::Resistances as i32),
                effect_num: Some(409),
                ..Default::default()
            }),
            action(ActEffect {
                target_id: Some(-1),
                effect_type: Some(EffectType::Expointchange as i32),
                effect_num: Some(-1),
                ..Default::default()
            }),
        ],
        ..Default::default()
    };

    assert_eq!(
        moxie_reduction_resistance_choices(&catalog, &round),
        vec![
            MoxieReductionResistanceChoice {
                producer_id: 30001,
                source_uid: 10,
                target_uid: -1,
                resisted: true,
            },
            MoxieReductionResistanceChoice {
                producer_id: 30001,
                source_uid: 10,
                target_uid: -1,
                resisted: false,
            },
        ]
    );
}

#[test]
fn direct_child_step_imports_the_parent_target_choice() {
    use battle::tooling::scan::{ParsedBehavior, ParsedSkillEffect, TargetRequest};
    use sonettobuf::{ActEffect, fight_hurt_info::DamageFromType};

    let parent_skill_id = 10;
    let child_skill_id = 20;
    let source_uid = 30;
    let target_uid = -2;
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: parent_skill_id,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                battle::tooling::scan::behavior::classify::BehaviorSpec::new(
                    50008,
                    "DirectUseSkill",
                ),
                vec![child_skill_id],
                vec![child_skill_id.to_string()],
            ),
            TargetRequest {
                code: 206,
                raw: Vec::new(),
            },
        )],
    });
    catalog.insert_logic_target(child_skill_id, 202);
    let hurt = |target_id| ActEffect {
        target_id: Some(target_id),
        hurt_info: Some(FightHurtInfo {
            damage_from_type: Some(DamageFromType::Skill as i32),
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_id: Some(parent_skill_id),
            from_id: Some(source_uid),
            act_effect: vec![ActEffect {
                fight_step: Some(FightStep {
                    act_id: Some(child_skill_id),
                    from_id: Some(source_uid),
                    to_id: Some(target_uid),
                    act_effect: vec![hurt(target_uid), hurt(-1)],
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let choices = skill_target_choices(&catalog, &round);

    assert_eq!(choices.len(), 2);
    assert_eq!(
        (
            choices[0].skill_id,
            choices[0].source_uid,
            choices[0].target_code,
            choices[0].targets.as_slice(),
        ),
        (parent_skill_id, source_uid, 206, [target_uid].as_slice())
    );
    assert_eq!(choices[1].skill_id, child_skill_id);
    assert_eq!(choices[1].targets, vec![target_uid, -1]);
}

#[test]
fn opening_rng_import_keeps_choices_and_excludes_temporary_cards() {
    crate::init_test_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                skill_group1: vec![101],
                skill_group2: vec![102],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                skill_group1: vec![201],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let normal = vec![
        card(10, 101, false),
        card(10, 102, false),
        card(10, 101, false),
        card(10, 102, false),
    ];
    let ai = vec![card(-1, 201, false)];
    let mut captured = normal.clone();
    captured.insert(2, card(10, 999, true));

    let mut determinism = opening_determinism(
        config::configs::get(),
        &fight,
        &FightRound {
            team_a_cards1: captured,
            ai_use_cards: ai.clone(),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        determinism.take_start_decks(),
        Some((ai, normal.clone(), normal, 0))
    );
}

#[test]
fn round_rng_import_excludes_only_existing_temporary_cards() {
    let after_actions = card(10, 101, false);
    let temporary = card(10, 999, true);
    let normalized_temporary = card(10, 999, false);
    let round_start = card(10, 102, false);

    assert_eq!(
        captured_round_draws(&FightRound {
            fight_step: vec![FightStep {
                act_effect: vec![sonettobuf::ActEffect {
                    effect_type: Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32,),
                    card_info_list: vec![temporary],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            team_a_cards2: vec![
                normalized_temporary.clone(),
                normalized_temporary.clone(),
                after_actions.clone(),
            ],
            team_a_cards1: vec![round_start.clone()],
            ..Default::default()
        }),
        vec![normalized_temporary, after_actions, round_start]
    );
}

#[test]
fn round_rng_import_requeues_a_removed_temporary_card() {
    let temporary = card(10, 999, true);
    let persisted = card(10, 999, false);

    assert_eq!(
        captured_round_draws(&FightRound {
            fight_step: vec![FightStep {
                act_effect: vec![
                    sonettobuf::ActEffect {
                        effect_type: Some(
                            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
                        ),
                        card_info_list: vec![temporary],
                        ..Default::default()
                    },
                    sonettobuf::ActEffect {
                        effect_type: Some(
                            sonettobuf::effect_type_enum::EffectType::Cardremove as i32,
                        ),
                        reserve_str: Some("1".to_owned()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            team_a_cards2: vec![persisted.clone()],
            ..Default::default()
        }),
        vec![persisted]
    );
}

#[test]
fn action_rng_import_does_not_queue_the_cloth_replys_ai_snapshot() {
    crate::init_test_config();
    let fight = Fight {
        version: Some(7),
        battle_id: Some(109380001),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100_000),
                skill_group1: vec![31050111],
                skill_group2: vec![31050121],
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                skill_group1: vec![40340111],
                skill_group2: vec![40340121],
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = ReplayBattle::new(
        battle::catalog::BattleCatalog::new(config::configs::get()),
        fight,
    )
    .unwrap();
    runtime.start_round().unwrap();
    let stale = card(-1, 40340111, false);
    let expected = card(-1, 40340121, false);

    seed_action_determinism(
        &mut runtime,
        &SkillEffectCatalog::default(),
        &FightRound {
            ai_use_cards: vec![stale],
            ..Default::default()
        },
    );
    seed_round_determinism(
        &mut runtime,
        &SkillEffectCatalog::default(),
        &FightRound {
            ai_use_cards: vec![expected.clone()],
            ..Default::default()
        },
    );

    let round = runtime.advance_round(Default::default()).unwrap();

    assert_eq!(
        round
            .ai_use_cards
            .iter()
            .map(|card| (card.uid, card.skill_id))
            .collect::<Vec<_>>(),
        vec![(expected.uid, expected.skill_id)]
    );
}

#[test]
fn observed_crits_follow_only_rolled_effects_of_their_own_step_in_order() {
    crate::init_test_config();
    use sonettobuf::{ActEffect, FightHurtInfo, effect_type_enum::EffectType};

    let effect = |effect_type: EffectType, target_id: i64, config_effect: i32| ActEffect {
        effect_type: Some(effect_type as i32),
        target_id: Some(target_id),
        config_effect: Some(config_effect),
        ..Default::default()
    };
    let nested = FightStep {
        act_id: Some(999),
        from_id: Some(30),
        act_effect: vec![
            ActEffect {
                effect_type: Some(EffectType::Buffupdate as i32),
                buff: Some(BuffInfo {
                    buff_id: Some(31090112),
                    from_uid: Some(20),
                    ..Default::default()
                }),
                ..Default::default()
            },
            effect(EffectType::Healcrit, 10, 0),
            effect(EffectType::Healcrit, 11, 0),
        ],
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_id: Some(31090111),
            from_id: Some(10),
            act_effect: vec![
                // Life loss and plain heals are not crit rolls.
                effect(EffectType::Damage, 10, 30006),
                effect(EffectType::Heal, 10, 0),
                effect(EffectType::Crit, -1, -1),
                effect(EffectType::Additionaldamage, -1, -1),
                ActEffect {
                    fight_step: Some(nested),
                    ..Default::default()
                },
                effect(EffectType::Additionaldamagecrit, -1, -1),
                effect(EffectType::Heal, 10, 20016),
                effect(EffectType::Damage, -2, -1),
                effect(EffectType::Heal, 10, 20001),
                effect(EffectType::Healcrit, 10, 60179),
                effect(EffectType::Buffadd, -2, 0),
                ActEffect {
                    effect_type: Some(EffectType::Eziobigskilldamage as i32),
                    target_id: Some(-1),
                    config_effect: Some(100002),
                    hurt_info: Some(FightHurtInfo {
                        hurt_effect: Some(EffectType::Crit as i32),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ActEffect {
                    effect_type: Some(EffectType::Eziobigskillorigindamage as i32),
                    target_id: Some(-1),
                    config_effect: Some(100003),
                    hurt_info: Some(FightHurtInfo {
                        hurt_effect: Some(EffectType::Damage as i32),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let (hidden, additional, indirect_heals) = crit_choices(config::configs::get(), &round);

    assert_eq!(
        hidden,
        vec![
            ((31090111, 10), true),
            ((31090111, 10), false),
            ((31090111, 10), false),
            ((31090111, 10), true),
            ((31090111, 10), true),
            ((31090111, 10), false)
        ]
    );
    assert_eq!(
        additional,
        vec![((31090111, 10, -1), false), ((31090111, 10, -1), true)]
    );
    assert_eq!(
        indirect_heals,
        vec![
            ((31090111, 10, 10), false),
            ((31090112, 20, 10), true),
            ((31090112, 20, 11), true),
        ]
    );
}

#[test]
fn shell_moxie_import_preserves_failed_and_successful_deployments() {
    crate::init_test_config();
    use sonettobuf::{ActEffect, effect_type_enum::EffectType};

    let shell = |buff_id, target_id, from_uid| ActEffect {
        effect_type: Some(EffectType::Buffupdate as i32),
        target_id: Some(target_id),
        buff: Some(BuffInfo {
            buff_id: Some(buff_id),
            from_uid: Some(from_uid),
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_effect: vec![
                // Retrieval updates the deployed shell before the stock and is not a roll.
                shell(31090112, -1, 20),
                shell(31090111, 20, -1),
                // Deployment updates the stock before the deployed shell and rolls once.
                shell(31090111, 20, -1),
                shell(31090112, -1, 20),
                shell(31090111, 20, -1),
                shell(31090112, -1, 20),
                ActEffect {
                    effect_type: Some(EffectType::Expointchange as i32),
                    target_id: Some(20),
                    effect_num: Some(1),
                    config_effect: Some(0),
                    ..Default::default()
                },
                shell(31090111, 20, -1),
                shell(31090112, -1, 20),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    assert_eq!(
        shell_moxie_choices(config::configs::get(), &round),
        vec![false, true, false]
    );
}

#[test]
fn crystal_reuse_import_preserves_failures_before_a_success() {
    crate::init_test_config();
    let parent = |child: Option<(i32, i64)>| FightStep {
        act_id: Some(31340151),
        from_id: Some(10),
        act_effect: child
            .into_iter()
            .map(|(skill_id, source_uid)| sonettobuf::ActEffect {
                fight_step: Some(FightStep {
                    act_id: Some(skill_id),
                    from_id: Some(source_uid),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![
            parent(Some((31340152, 30))),
            FightStep {
                act_id: Some(31340151),
                from_id: Some(10),
                act_effect: vec![sonettobuf::ActEffect {
                    fight_step: Some(FightStep {
                        act_id: Some(1),
                        from_id: Some(10),
                        act_effect: vec![sonettobuf::ActEffect {
                            fight_step: Some(FightStep {
                                act_id: Some(31340152),
                                from_id: Some(10),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            },
            parent(None),
            parent(Some((31340152, 10))),
        ],
        ..Default::default()
    };

    assert_eq!(
        crystal_reuse_choices(battle::tooling::scan::effect_catalog::global(), &round),
        vec![
            CrystalReuseChoice {
                source_uid: 10,
                skill_id: 31340151,
                result_skill_id: 31340152,
                success: false,
            },
            CrystalReuseChoice {
                source_uid: 10,
                skill_id: 31340151,
                result_skill_id: 31340152,
                success: false,
            },
            CrystalReuseChoice {
                source_uid: 10,
                skill_id: 31340151,
                result_skill_id: 31340152,
                success: false,
            },
            CrystalReuseChoice {
                source_uid: 10,
                skill_id: 31340151,
                result_skill_id: 31340152,
                success: true,
            },
        ]
    );
}

#[test]
fn card_energy_import_keeps_allocations_from_both_step_lists() {
    use sonettobuf::effect_type_enum::EffectType;
    let record = |allocated: Option<i32>, energy| sonettobuf::ActEffect {
        effect_type: Some(EffectType::Allocatecardenergy as i32),
        effect_num1: allocated,
        card_info_list: vec![CardInfo {
            uid: Some(1),
            skill_id: Some(10),
            energy: Some(energy),
            ..Default::default()
        }],
        ..Default::default()
    };
    let step = |effects| FightStep {
        act_effect: effects,
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![step(vec![record(None, 0), record(Some(1), 2)])],
        next_round_begin_step: vec![step(vec![record(Some(1), 3)])],
        ..Default::default()
    };

    assert_eq!(
        card_energy_allocations(&round)
            .iter()
            .map(|hand| hand[0].energy)
            .collect::<Vec<_>>(),
        vec![Some(2), Some(3)]
    );
}
