use super::*;

#[test]
fn stage_102_duration_expires_before_condition_102_actions() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(4030703),
                current_hp: Some(100),
                team_type: Some(2),
                passive_skill: vec![2524],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(25332),
                    from_uid: Some(-1),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [2524], []);
    let mut managers = BattleManagers::seeded(&fight);

    let (round, _) = run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 5,
            ..Default::default()
        },
        2,
    )
    .unwrap();

    let expiry = round
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffBatch(changes)
                    if changes.iter().any(|change| {
                        change.origin.key.opcode == 102
                            && change
                                .change
                                .removed
                                .iter()
                                .any(|removed| removed.buff.uid == Some(20))
                    })
            )
        })
        .expect("stage 102 removes the snapshotted one-round buff");
    let condition = round
        .outcomes
        .iter()
        .position(|outcome| match outcome {
            RuleOutcome::Buff(change) => change
                .change
                .added
                .iter()
                .any(|added| added.buff.buff_id == Some(25241)),
            RuleOutcome::BuffBatch(changes) => changes.iter().any(|change| {
                change
                    .change
                    .added
                    .iter()
                    .any(|added| added.buff.buff_id == Some(25241))
            }),
            _ => false,
        })
        .expect("condition 102 executes its configured passive");

    assert!(expiry < condition);
    assert!(!managers.buff.has_buff_id(-1, 25332));
}

#[test]
fn ulrich_channel_reacts_before_take_stage_104_expires_from_the_round_snapshot() {
    init_config();
    let entity = |uid, team_type, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        team_type: Some(team_type),
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    1,
                    vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31070121),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    }],
                ),
                entity(11, 1, Vec::new()),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add_special_count(10, &[31070121], 3);

    let (round, _) = run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
    )
    .unwrap();

    let channel_reaction = round
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffFeatureMarker(marker)
                    if marker.target_uid == 10
                        && marker.effect_type
                            == sonettobuf::effect_type_enum::EffectType::Triggeranalysis as i32
            )
        })
        .expect("the channel reacts while its source buff is active");
    let channel_expiry = round
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffBatch(changes)
                    if changes.iter().any(|change| {
                        change.origin.key.opcode == 104
                            && change
                                .change
                                .removed
                                .iter()
                                .any(|removed| removed.buff.uid == Some(20))
                    })
            )
        })
        .expect("takeStage 104 expires the one-round channel");
    assert!(channel_reaction < channel_expiry);
    assert!(!managers.buff.has_buff_id(10, 31070121));

    for owner_uid in [10, 11] {
        let output = managers
            .buff
            .active_for(owner_uid)
            .find(|buff| buff.buff_id == Some(31070151))
            .expect("the channel grants its configured ally output");
        assert_eq!(output.duration, Some(1));
    }
}

#[test]
fn jiu_tipsy_casts_then_releases_its_channel() {
    init_config();
    let entity = |uid, team_type, model_id, current_hp, buffs| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(current_hp),
        team_type: Some(team_type),
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
        attacker: Some(FightTeam {
            entitys: vec![entity(
                10,
                1,
                3083,
                10_000,
                vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30830131),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
            )],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, 1000, 5_000, Vec::new()),
                entity(-2, 2, 1001, 9_000, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30830111], [30830131]);
    let mut managers = BattleManagers::seeded(&fight);

    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(10, 30830131));
    assert!(managers.hp.current(-1) < 5_000);
    assert_eq!(managers.hp.current(-2), 9_000);
}

#[test]
fn voiceless_switches_afflatus_weakness_after_recovery() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    for starts_weak in [false, true] {
        let mut buffs = vec![BuffInfo {
            uid: Some(1),
            buff_id: Some(109360006),
            from_uid: Some(-1),
            ..Default::default()
        }];
        if starts_weak {
            buffs.push(BuffInfo {
                uid: Some(2),
                buff_id: Some(109360007),
                from_uid: Some(-1),
                ..Default::default()
            });
        }
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    passive_skill: vec![109360006],
                    buffs,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);

        run_round_start_split(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 2,
                ..Default::default()
            },
            2,
        )
        .unwrap();

        assert_eq!(managers.buff.has_buff_id(-1, 109360007), !starts_weak);
        assert!(!managers.buff.has_buff_id(-1, 109360006));
        assert!(!managers.buff.has_buff_id(-1, 109360008));
    }
}

#[test]
fn round_start_settles_unlisted_capacity_owner_before_its_after_settlement_act() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(99),
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
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 99, 99, 31250151, 0);
    managers.buff.add(&managers.hp, 99, 99, 31260121, 0);
    let features = managers.buff.active_features(&managers.hp);
    let raspberry = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::Raspberry))
        .unwrap();
    let capacity_groups = crate::engine::mechanic::shadow_cloak::capacity_rule_groups(
        &managers,
        &features,
        &std::collections::HashMap::from([((99, raspberry.buff_uid), 100)]),
    )
    .unwrap();
    let plan = RoundStartSettlementPlan::new(capacity_groups, vec![10]);

    let result = run_round_start_owner_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        plan,
    )
    .unwrap();

    let capacity = result
        .outcomes
        .iter()
        .position(|outcome| matches!(outcome, RuleOutcome::RaspberryCapacity(_)))
        .unwrap();
    let after_settlement_loss = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Hp(execution)
                    if execution
                        .changes
                        .hp
                        .is_some_and(|hp| hp.target_uid == 99 && hp.delta < 0)
            )
        })
        .unwrap();
    assert!(capacity < after_settlement_loss);
}

#[test]
fn round_start_capacity_uses_only_exact_raspberry_loss_instances() {
    use crate::engine::manager::hp::{HpChange, HpChanges, HurtDamageFromType, HurtInfoData};

    let hp_outcome = |domain, key, delta| {
        RuleOutcome::Hp(Box::new(crate::engine::manager::HpExecution {
            changes: HpChanges {
                origin: CommandOrigin { domain, key },
                source_uid: 10,
                target_uid: 10,
                publishes_hit: true,
                damage: None,
                fake_hp_consumed: None,
                team_shared_shield_absorbed: None,
                team_shared_shield_removed: None,
                shared_hurt: None,
                shield_absorbed: None,
                shield_granted: None,
                max_hp: None,
                hp: Some(HpChange {
                    target_uid: 10,
                    before: 100,
                    delta,
                    after: 100 + delta,
                    max: 100,
                    config_effect: 0,
                    hurt: Some(HurtInfoData {
                        from_uid: 10,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: 0,
                        effect_id: 0,
                        skill_id: 0,
                        damage_from: HurtDamageFromType::Buff,
                        buff_act_id: 1042,
                        buff_uid: 20,
                        hurt_effect_type: 0,
                        display_amount: None,
                    }),
                    assassinate: false,
                    effect_type: 0,
                    display_amount: None,
                }),
                toughness: None,
                kill: None,
                death: None,
            },
            before_hp_markers: Vec::new(),
            indicator: None,
        }))
    };
    let result = DrainResult {
        outcomes: vec![
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1042, "Raspberry"),
                -10,
            ),
            hp_outcome(
                RuleDomain::Behavior,
                DefinitionKey::new(1042, "Raspberry"),
                -50,
            ),
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1041, "RaspberryBigSkill"),
                -60,
            ),
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1042, "Raspberry"),
                -15,
            ),
        ],
        ..Default::default()
    };

    assert_eq!(
        raspberry_losses(&result),
        std::collections::HashMap::from([((10, 20), 25)])
    );
}

#[test]
fn defender_round_start_expires_the_previous_status_before_alternating_setup() {
    init_config();
    let entity = |uid, model_id, passive_skill, buffs| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        passive_skill,
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                entity(
                    -2,
                    900016101,
                    vec![811453],
                    vec![BuffInfo {
                        uid: Some(1076),
                        buff_id: Some(2112021),
                        from_uid: Some(-2),
                        duration: Some(1),
                        ..Default::default()
                    }],
                ),
                entity(-3, 900016102, Vec::new(), Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        2,
        &[],
        None,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(-2, 2112021));
    let active = managers
        .buff
        .active_for(-3)
        .filter(|buff| buff.buff_id == Some(2112021))
        .collect::<Vec<_>>();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].from_uid, Some(-2));
}

#[test]
fn round_start_resolves_field_with_configured_allied_threshold_modifiers() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31280117),
                        from_uid: Some(20),
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50019, "AddMagicCircle"),
    };
    let thresholds =
        crate::engine::skill::behavior::magic_circle::field_thresholds(30001, 1, &managers);
    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds,
            },
        })
        .unwrap();
    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 80 },
        })
        .unwrap();

    let (first_round, _) = run_round_start_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();
    assert!(first_round.frames.iter().any(|frame| {
        matches!(
            frame.items.as_slice(),
            [crate::engine::runtime::record::FrameItem::Cue(
                RoundCue::ChangeRound { .. }
            )]
        )
    }));
    assert_eq!(managers.field.get(1).unwrap().level, 1);

    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 20 },
        })
        .unwrap();
    run_round_start_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
    )
    .unwrap();
    let field = managers.field.get(1).unwrap();
    assert_eq!(field.level, 3);
    assert_eq!(field.definition.field_id, 30003);
    assert_eq!(field.next_upgrade_progress, 120);
}

fn scheduled_round_start(version: i32, model_id: Option<i32>, is_record: bool) -> DrainResult {
    init_config();
    let fight = Fight {
        version: Some(version),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id,
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        is_record: Some(is_record),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 3,
            ..Default::default()
        },
        &[],
        0,
    )
    .unwrap()
    .0
}

fn change_round_effect_num(round: &DrainResult, version: i32) -> i32 {
    let steps =
        crate::engine::packet::timeline::project_for_version(&round.frames, version).unwrap();
    steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Changeround as i32)
        })
        .and_then(|effect| effect.effect_num)
        .expect("round start emits a CHANGE_ROUND marker")
}

#[test]
fn live_version_seven_change_round_payload_names_the_active_round() {
    let round = scheduled_round_start(7, None, false);
    assert_eq!(change_round_effect_num(&round, 7), 3);
    assert!(round.outcomes.iter().all(|outcome| !matches!(
        outcome,
        RuleOutcome::Conduit(
            crate::engine::manager::conduit::ConduitChange::PowersReset { team: 1 }
        )
    )));
    assert!(round.frames.iter().any(|frame| matches!(
        frame.owner,
        crate::engine::runtime::record::FrameOwner::RoundPhase(
            crate::engine::runtime::record::RoundPhase::RoundStartSettlement
        )
    )));
}

#[test]
fn change_round_payload_is_zero_when_the_layout_does_not_reset_conduit_power() {
    let round = scheduled_round_start(6, Some(3149), false);
    assert_eq!(change_round_effect_num(&round, 6), 0);
    assert!(round.outcomes.iter().all(|outcome| !matches!(
        outcome,
        RuleOutcome::Conduit(crate::engine::manager::conduit::ConduitChange::PowersReset { .. })
    )));
}

#[test]
fn live_version_seven_change_round_payload_is_independent_of_conduit_power() {
    let round = scheduled_round_start(7, Some(3149), false);
    assert_eq!(change_round_effect_num(&round, 7), 3);
    let conduit_changes = round
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Conduit(change) => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        conduit_changes.as_slice(),
        [
            crate::engine::manager::conduit::ConduitChange::PowersReset { team: 1 },
            crate::engine::manager::conduit::ConduitChange::DeviceRestarted {
                source_uid: 10,
                team: 1,
            },
        ]
    ));
}

#[test]
fn recorded_version_seven_change_round_payload_uses_the_v7_projection() {
    let round = scheduled_round_start(7, None, true);
    assert_eq!(change_round_effect_num(&round, 7), 3);
}

#[test]
fn round_transition_renews_timed_team_attributes_on_reserves() {
    init_config();
    let old_attribute_uid = 22;
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31340003),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31340008),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(11),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(old_attribute_uid),
                    buff_id: Some(31340007),
                    from_uid: Some(0),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestLingeringGlow"),
    };
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 45 },
        ))
        .unwrap();

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();

    let renewed = round
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(result) => result.change.added.as_ref(),
            _ => None,
        })
        .filter(|added| added.buff.buff_id == Some(31340007))
        .map(|added| (added.target_uid, added.buff.uid.unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(renewed.len(), 2);
    assert_eq!(
        renewed
            .iter()
            .map(|(target_uid, _)| *target_uid)
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
    assert_ne!(renewed[0].1, renewed[1].1);
    assert!(managers.buff.snapshot(11, old_attribute_uid).is_none());
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .filter(|buff| buff.buff_id == Some(31340007))
            .count(),
        1
    );
    assert_eq!(
        managers
            .buff
            .active_for(11)
            .filter(|buff| buff.buff_id == Some(31340007))
            .count(),
        1
    );
    assert!(
        managers
            .buff
            .active_for(11)
            .any(|buff| buff.buff_id == Some(31340007) && buff.uid != Some(old_attribute_uid))
    );
    let next = managers.buff.add(&managers.hp, 10, 10, 101, 1).unwrap();
    assert_eq!(next.buff.uid, Some(1006));
}
