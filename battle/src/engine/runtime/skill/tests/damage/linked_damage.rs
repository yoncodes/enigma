use super::*;

#[test]
fn nested_assassination_cast_derives_its_own_reaction_cardinality() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31240121),
                    from_uid: Some(10),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31243121, 312431212], []);
    let mut parent = SkillExecution::new(TargetContext::default());
    plan::snapshot_action_start_damage_state(&managers, &catalog, 31243121, &mut parent).unwrap();
    assert_eq!(
        parent.assassination_rate_marks_at_action_start,
        Some(vec![])
    );

    let consumed = BattleManagers::seeded(&Fight::default());
    let mut child = SkillExecution::new(TargetContext::default());
    child.inherit_assassination_marks(&parent);
    plan::snapshot_action_start_damage_state(&consumed, &catalog, 312431212, &mut child).unwrap();
    assert_eq!(
        child.assassination_rate_marks_at_action_start,
        Some(vec![(-1, 10)])
    );
}

#[test]
fn resolved_hit_additional_damage_precedes_independent_linked_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        technic: Some(300),
                        ..Default::default()
                    }),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31050144),
                            from_uid: Some(12),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(312451456),
                            from_uid: Some(11),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        technic: Some(500),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    technic: Some(0),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(31240121),
                    from_uid: Some(11),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    const SKILL_ID: i32 = 999_999_006;
    catalog.insert_damage_rate(SKILL_ID, 6_000);
    catalog.insert_logic_target(SKILL_ID, 1);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);
    execution.assassination_marks_at_action_start =
        Some(crate::engine::skill::buff_act::assassination::marked_targets(&managers));
    for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
        execution.activate_additional_damage(activation.additional);
    }
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(SKILL_ID, 10, [false]);
    determinism.enqueue_additional_crits(SKILL_ID, 10, -1, [false, false]);

    let hits = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut determinism,
        &mut execution,
    )
    .additional_damage
    .into_iter()
    .filter_map(|command| match command {
        HpCommand::Damage(hit) => Some((hit.source_uid, hit.hurt.is_crit, hit.amount)),
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|(_, is_crit, _)| !is_crit));
    assert_eq!(
        hits.iter()
            .map(|(source_uid, _, _)| *source_uid)
            .collect::<Vec<_>>(),
        vec![10, 12]
    );
    // The independently rolled hit belongs to uid 12, so its five complete Technique bands
    // provide Final DMG +25%; it must not inherit uid 10's +15% modifier.
    assert_eq!(
        hits.iter()
            .find_map(|(source_uid, _, amount)| (*source_uid == 12).then_some(*amount)),
        Some(375)
    );
}

#[test]
fn linked_damage_main_rate_follows_the_explicit_action_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31050144),
                        from_uid: Some(12),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    const SKILL_ID: i32 = 999_999_008;
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    catalog.insert_logic_target(SKILL_ID, 1);
    let linked_damage = |selected_target| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: SKILL_ID,
        }
        .into();
        invocation.target = SkillTarget::Explicit(selected_target);
        let mut execution = SkillExecution::new(TargetContext::default());
        execution.configured_targets = Some(vec![-1]);
        for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
            execution.activate_additional_damage(activation.additional);
        }
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(SKILL_ID, 10, [false]);
        determinism.enqueue_additional_crits(SKILL_ID, 10, -1, [false]);
        plan::damage_ops(
            &invocation,
            &managers,
            &pool,
            &catalog,
            SKILL_ID,
            &mut determinism,
            &mut execution,
        )
        .additional_damage
        .into_iter()
        .find_map(|command| match command {
            HpCommand::Damage(hit) => Some(hit.amount),
            _ => None,
        })
        .expect("force field should add linked damage")
    };

    let enemy_selected = linked_damage(-1);
    let ally_selected = linked_damage(11);

    assert_eq!(enemy_selected, ally_selected * 2);
}

#[test]
fn extra_action_penalties_follow_their_target_and_source_owners() {
    crate::test_support::init_config();
    let resolve = |with_target_passive: bool, with_source_penalty: bool| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        career: Some(6),
                        current_hp: Some(10_000),
                        attr: Some(HeroAttribute {
                            hp: Some(10_000),
                            attack: Some(1_000),
                            technic: Some(300),
                            ..Default::default()
                        }),
                        buffs: vec![
                            BuffInfo {
                                uid: Some(20),
                                buff_id: Some(31050144),
                                from_uid: Some(12),
                                ..Default::default()
                            },
                            BuffInfo {
                                uid: Some(21),
                                buff_id: Some(109380006),
                                from_uid: Some(-1),
                                ..Default::default()
                            },
                        ]
                        .into_iter()
                        .chain(with_source_penalty.then_some(BuffInfo {
                            uid: Some(22),
                            buff_id: Some(109380002),
                            from_uid: Some(-1),
                            ..Default::default()
                        }))
                        .collect(),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(12),
                        current_hp: Some(10_000),
                        attr: Some(HeroAttribute {
                            hp: Some(10_000),
                            attack: Some(1_000),
                            technic: Some(500),
                            ..Default::default()
                        }),
                        power_infos: vec![sonettobuf::PowerInfo {
                            power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
                            num: Some(5),
                            max: Some(5),
                        }],
                        buffs: with_source_penalty
                            .then_some(BuffInfo {
                                uid: Some(23),
                                buff_id: Some(109380002),
                                from_uid: Some(-1),
                                ..Default::default()
                            })
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    career: Some(5),
                    current_hp: Some(100_000),
                    attr: Some(HeroAttribute {
                        hp: Some(100_000),
                        technic: Some(0),
                        ..Default::default()
                    }),
                    passive_skill: if with_target_passive {
                        vec![109380003, 23390182]
                    } else {
                        vec![23390182]
                    },
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        const SKILL_ID: i32 = 31130114;
        let mut catalog = SkillEffectCatalog::from_roots(
            config::configs::get(),
            [109380003, 23390182, SKILL_ID],
            [],
        );
        catalog.insert_damage_rate(SKILL_ID, 6_000);
        catalog.insert_logic_target(SKILL_ID, 1);
        let invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: SKILL_ID,
        }
        .into();
        let mut execution = SkillExecution::new(TargetContext {
            active_skill_mode: SkillExecutionMode::Nested,
            extra_skill_kind: crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction
                .id(),
            target_observed_extra_action: true,
            ..Default::default()
        });
        execution.configured_targets = Some(vec![-1]);
        for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
            execution.activate_additional_damage(activation.additional);
        }
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(SKILL_ID, 10, [true]);
        determinism.enqueue_additional_crits(SKILL_ID, 10, -1, [true]);
        let ops = plan::damage_ops(
            &invocation,
            &managers,
            &pool,
            &catalog,
            SKILL_ID,
            &mut determinism,
            &mut execution,
        );
        let [HpCommand::Damage(main)] = ops.damage.as_slice() else {
            panic!("expected one main damage command")
        };
        let [HpCommand::Damage(linked)] = ops.additional_damage.as_slice() else {
            panic!("expected one linked damage command")
        };
        (main.amount, linked.amount)
    };

    let baseline = resolve(false, false);
    let target_penalized = resolve(true, false);
    let source_penalized = resolve(false, true);

    assert!(target_penalized.0 < baseline.0);
    assert!(target_penalized.1 < baseline.1);
    assert!(source_penalized.0 < baseline.0);
    assert_eq!(source_penalized.1, baseline.1);
}

#[test]
fn a_gust_force_field_extra_action_opens_with_its_cost_and_forced_crit_marker() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(5),
                }],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31050146),
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
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451011], []);
    let damage_phase = |eureka, card_index| {
        let mut fight = fight.clone();
        fight.attacker.as_mut().unwrap().entitys[0].power_infos[0].num = Some(eureka);
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            10,
            &HeroExAttribute {
                cri: Some(400),
                cri_dmg: Some(1_750),
                ..Default::default()
            },
        );
        let pool = TargetPool::from_fight(&fight);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 312451011,
        }
        .into();
        invocation.mode = SkillExecutionMode::Active;
        invocation.card_index = card_index;
        invocation.target = SkillTarget::Explicit(-1);
        let mut execution = SkillExecution::new(TargetContext::default());
        let mut determinism = RoundDeterminism::default();
        let mut ops = Vec::new();
        let mut next = Some(invocation);
        while let Some(invocation) = next.take() {
            let emission = emit_ops(
                invocation,
                &managers,
                &pool,
                &catalog,
                &mut determinism,
                &mut execution,
                &SkillOpTrigger::Active,
            )
            .unwrap();
            let hits = emission
                .ops
                .iter()
                .any(|emission| matches!(emission.op, RuleOp::Command(BattleCommand::HpBatch(_))));
            ops.extend(emission.ops);
            if hits {
                let main = ops
                    .iter()
                    .find_map(|emission| match &emission.op {
                        RuleOp::Command(BattleCommand::HpBatch(commands)) => {
                            commands.iter().find_map(|command| match command {
                                HpCommand::Damage(damage) => Some(*damage),
                                _ => None,
                            })
                        }
                        _ => None,
                    })
                    .expect("damage phase should contain the main hit");
                return (ops, execution.pending_additional_damage, main);
            }
            next = emission.continuation;
        }
        panic!("no damage phase");
    };
    let opens_with_cost_then_marker = |ops: &[SkillEmissionOp]| {
        let position = |matches: &dyn Fn(&RuleOp) -> bool| {
            ops.iter().position(|emission| matches(&emission.op))
        };
        let cost = position(&|op| matches!(op, RuleOp::Command(BattleCommand::Eureka(_))));
        let marker = position(&|op| {
            matches!(
                op,
                RuleOp::EffectMarker { target_uid: 10, effect_type, reserve_id: Some(-1), .. }
                    if *effect_type == sonettobuf::effect_type_enum::EffectType::Mustcrit as i32
            )
        });
        let own_effect = position(&|op| matches!(op, RuleOp::Command(BattleCommand::Buff(_))));
        let hits = position(&|op| matches!(op, RuleOp::Command(BattleCommand::HpBatch(_))));
        matches!(
            (cost, marker, own_effect, hits),
            (Some(cost), Some(marker), Some(own_effect), Some(hits))
                if cost < marker && marker < own_effect && own_effect < hits
        )
    };

    // "The extra action is always a critical hit" applies to the action itself. Linked damage
    // credited to another producer keeps that producer's independently imported critical roll.
    let (paid_ops, paid_additional, paid_main) = damage_phase(2, 0);
    assert!(opens_with_cost_then_marker(&paid_ops));
    assert!(!paid_additional.is_empty());
    assert!(paid_additional.iter().all(|command| matches!(
        command,
        HpCommand::Damage(damage)
            if damage.effect_kind == crate::engine::manager::hp::DamageEffectKind::Normal
    )));
    let (unpaid_ops, unpaid_additional, unpaid_main) = damage_phase(0, 0);
    assert!(unpaid_main.hurt.is_crit);
    assert_eq!(paid_main.amount, unpaid_main.amount);
    assert!(
        !unpaid_ops
            .iter()
            .any(|emission| matches!(emission.op, RuleOp::Command(BattleCommand::Eureka(_))))
    );
    assert!(unpaid_ops.iter().any(|emission| matches!(
        emission.op,
        RuleOp::EffectMarker {
            effect_type,
            reserve_id: Some(-1),
            ..
        } if effect_type == sonettobuf::effect_type_enum::EffectType::Mustcrit as i32
    )));
    assert!(!unpaid_additional.is_empty());
    let (ops, _, _) = damage_phase(2, 1);
    assert!(
        !ops.iter()
            .any(|emission| matches!(emission.op, RuleOp::EffectMarker { .. }))
    );
}

#[test]
fn unpaid_force_field_keeps_its_immediate_origin_damage_bonus() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
                    num: Some(0),
                    max: Some(5),
                }],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31050146),
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
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(400),
            cri_dmg: Some(1_750),
            ..Default::default()
        },
    );
    let pool = TargetPool::from_fight(&fight);
    const SKILL_ID: i32 = 999_999_105;
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: SKILL_ID,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30015, "OriginDamageCanCrit"),
                vec![0, AttrId::Attack.id(), 1_000],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

    let ops = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            active_skill_mode: SkillExecutionMode::Active,
            ..Default::default()
        },
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(ops.iter().any(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            amount: 2_275,
            hurt: Some(hurt),
            ..
        }))) if hurt.is_crit
    )));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, RuleOp::Command(BattleCommand::Eureka(_))))
    );
}

#[test]
fn an_extra_action_resolves_its_extra_action_passives_before_the_force_field_cost() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![433911],
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(crate::engine::manager::eureka::EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(5),
                }],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31050146),
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
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31050151, 433911], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31050151,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);
    invocation.action_hand_settlement = Some(FIELD_ORIGIN);
    let mut execution = SkillExecution::new(TargetContext::default());
    let mut determinism = RoundDeterminism::default();
    let started = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    // Psychube 433911 "When the carrier performs an extra action" resolves as the action starts.
    assert!(matches!(
        started.ops.as_slice(),
        [SkillEmissionOp {
            op: RuleOp::Publish(crate::engine::event::payload::BattleEvent::SkillEffectStarted(
                started
            )),
            ..
        }] if started.hand_settlement.is_some()
    ));
    let resumed = emit_ops(
        started
            .continuation
            .expect("extra action resumes after its start event"),
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(
        !resumed
            .ops
            .iter()
            .any(|emission| matches!(emission.op, RuleOp::ActionHandSettlement { .. }))
    );
    assert!(
        resumed
            .ops
            .iter()
            .any(|emission| { matches!(emission.op, RuleOp::Command(BattleCommand::Eureka(_))) })
    );
    assert_eq!(
        resumed
            .ops
            .iter()
            .filter(|emission| matches!(
                emission.op,
                RuleOp::EffectMarker { effect_type, .. }
                    if effect_type == sonettobuf::effect_type_enum::EffectType::Mustcrit as i32
            ))
            .count(),
        1
    );
}
