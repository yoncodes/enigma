use super::*;
use crate::engine::skill::condition::extra::ExtraSkillKind;

#[test]
fn target_passive_and_attack_local_attribute_are_applied_to_damage() {
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
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(30920155),
                    from_uid: Some(10),
                    count: Some(1),
                    ..Default::default()
                }],
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
                passive_skill: vec![70009],
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31040005),
                    from_uid: Some(10),
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
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    const SKILL_ID: i32 = 999_999_001;
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let mut determinism = RoundDeterminism::default();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut determinism,
        &mut execution,
    );

    assert!(matches!(
        damage.damage.as_slice(),
        [HpCommand::Damage(crate::engine::manager::hp::HpDamage {
            amount: 880,
            ..
        })]
    ));
}

#[test]
fn source_passive_attack_modifier_is_collected_by_runtime_damage() {
    const SKILL_ID: i32 = 999_999_002;
    const PASSIVE_ID: i32 = 999_999_003;
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
                passive_skill: vec![PASSIVE_ID],
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
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    catalog.insert(ParsedSkillEffect {
        skill_id: PASSIVE_ID,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(10004, "AttrFix", vec![AttrId::DmgBonus as i32, 100]),
            TargetRequest {
                code: crate::engine::skill::target::request::SOURCE_TARGET_CODE,
                raw: Vec::new(),
            },
        )],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut RoundDeterminism::default(),
        &mut execution,
    );

    assert!(matches!(
        damage.damage.as_slice(),
        [HpCommand::Damage(crate::engine::manager::hp::HpDamage {
            amount: 1_100,
            ..
        })]
    ));
}

#[test]
fn paid_action_damage_uses_committed_source_and_target_state() {
    crate::test_support::init_config();
    const SKILL_ID: i32 = 999_999_008;
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(434421),
                    from_uid: Some(10),
                    layer: Some(4),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri_dmg: Some(1_400),
            ..Default::default()
        },
    );
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    catalog.insert(ParsedSkillEffect {
        skill_id: SKILL_ID,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::with_payment(
        TargetContext::default(),
        Some(crate::engine::skill::action::ActionPayment {
            cost: crate::engine::manager::ex_point::ExPointCommand::Spend(
                crate::engine::manager::ex_point::ExPointChange {
                    origin: CommandOrigin {
                        domain: RuleDomain::Skill,
                        key: DefinitionKey::new(0, "TestPaidAction"),
                    },
                    source_uid: 10,
                    target_uid: 10,
                    delta: -5,
                    config_effect: 0,
                    effect_type: 0,
                },
            ),
            composition: Vec::new(),
        }),
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(SKILL_ID, 10, [true]);
    let mut damage = None;

    loop {
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
        if emission.ops.iter().any(|emission| {
            matches!(
                &emission.op,
                RuleOp::BeginSkillAction {
                    lifecycle: crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
                    ..
                } if action.phase == SkillPhase::Immediate
            )
        }) {
            managers.buff.add_replacing_excluded_with_layer_specified(
                &managers.hp,
                10,
                10,
                434421,
                5,
                true,
            );
            managers.buff.add_replacing_excluded_with_layer_specified(
                &managers.hp,
                -1,
                -1,
                204,
                1,
                true,
            );
            managers.attribute.override_ex(
                -1,
                &HeroExAttribute {
                    cri_def: Some(200),
                    ..Default::default()
                },
            );
        }
        damage = emission
            .ops
            .iter()
            .find_map(|emission| match &emission.op {
                RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))) => {
                    Some(damage.amount)
                }
                RuleOp::Command(BattleCommand::HpBatch(commands)) => {
                    commands.iter().find_map(|command| match command {
                        HpCommand::Damage(damage) => Some(damage.amount),
                        _ => None,
                    })
                }
                _ => None,
            })
            .or(damage);
        let Some(continuation) = emission.continuation else {
            break;
        };
        invocation = continuation;
    }

    assert_eq!(managers.buff.buff_id_amount(10, 434421), 9);
    assert!(managers.buff.has_buff_id(-1, 204));
    assert_eq!(managers.attribute.get(-1, AttrId::CriticalDef), 200);
    assert_eq!(damage, Some(1_140));
}

#[test]
fn configured_single_and_mass_attack_penalties_follow_the_skill_target_count() {
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
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(3091),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(3101),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2]
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .to_vec(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    let damage_amounts = |logic_target| {
        let mut catalog = SkillEffectCatalog::default();
        const SKILL_ID: i32 = 999_999_004;
        catalog.insert(ParsedSkillEffect {
            skill_id: SKILL_ID,
            slots: Vec::new(),
        });
        catalog.insert_damage_rate(SKILL_ID, 1_000);
        catalog.insert_logic_target(SKILL_ID, logic_target);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: SKILL_ID,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Active,
        )
        .unwrap()
        .into_iter()
        .flat_map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))) => {
                vec![damage.amount]
            }
            RuleOp::Command(BattleCommand::HpBatch(commands)) => commands
                .into_iter()
                .filter_map(|command| match command {
                    HpCommand::Damage(damage) => Some(damage.amount),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>()
    };

    assert_eq!(damage_amounts(1), vec![700]);
    assert_eq!(damage_amounts(201), vec![700, 700]);
}

#[test]
fn typed_be_attacked_reduction_uses_the_attackers_damage_type() {
    crate::test_support::init_config();
    let damage_amount = |source_model_id| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(source_model_id),
                    entity_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
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
                    buffs: vec![BuffInfo {
                        uid: Some(3),
                        buff_id: Some(3131),
                        from_uid: Some(-1),
                        count: Some(1),
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
        const SKILL_ID: i32 = 999_999_005;
        catalog.insert(ParsedSkillEffect {
            skill_id: SKILL_ID,
            slots: Vec::new(),
        });
        catalog.insert_damage_rate(SKILL_ID, 1_000);
        catalog.insert_logic_target(SKILL_ID, 1);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: SKILL_ID,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);

        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Active,
        )
        .unwrap()
        .into_iter()
        .find_map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))) => Some(damage.amount),
            RuleOp::Command(BattleCommand::HpBatch(commands)) => {
                commands.into_iter().find_map(|command| match command {
                    HpCommand::Damage(damage) => Some(damage.amount),
                    _ => None,
                })
            }
            _ => None,
        })
        .unwrap()
    };

    assert_eq!(damage_amount(3070), 1_250);
    assert_eq!(damage_amount(3102), 1_000);
}

#[test]
fn linked_damage_uses_its_credited_sources_damage_type() {
    crate::test_support::init_config();
    let damage_pair = |target_buffs| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        model_id: Some(3102),
                        entity_type: Some(1),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: vec![BuffInfo {
                            uid: Some(1),
                            buff_id: Some(31050144),
                            from_uid: Some(20),
                            count: Some(1),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        model_id: Some(3070),
                        entity_type: Some(1),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
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
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: target_buffs,
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
        catalog.insert_damage_rate(SKILL_ID, 1_000);
        let invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: SKILL_ID,
        }
        .into();
        let mut execution = SkillExecution::new(TargetContext::default());
        execution.configured_targets = Some(vec![-1]);
        for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
            execution.activate_additional_damage(activation.additional);
        }
        let damage = plan::damage_ops(
            &invocation,
            &managers,
            &pool,
            &catalog,
            SKILL_ID,
            &mut RoundDeterminism::default(),
            &mut execution,
        );
        let [HpCommand::Damage(main)] = damage.damage.as_slice() else {
            panic!("expected main damage")
        };
        let [HpCommand::Damage(linked)] = damage.additional_damage.as_slice() else {
            panic!("expected linked damage")
        };
        (main.amount, linked.amount)
    };
    let baseline = damage_pair(Vec::new());
    let typed = damage_pair(vec![BuffInfo {
        uid: Some(3),
        buff_id: Some(3131),
        from_uid: Some(-1),
        count: Some(1),
        ..Default::default()
    }]);

    assert_eq!(typed.0, baseline.0);
    assert!(typed.1 > baseline.1);
}

#[test]
fn captured_additional_target_order_does_not_control_damage() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut source = entity(10);
    source.buffs.push(BuffInfo {
        uid: Some(1),
        buff_id: Some(31260151),
        from_uid: Some(10),
        count: Some(2),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    const SKILL_ID: i32 = 999_999_004;
    catalog.insert(ParsedSkillEffect {
        skill_id: SKILL_ID,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    catalog.insert_logic_target(SKILL_ID, 202);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: SKILL_ID,
            source_uid: 10,
            target_code: 202,
            targets: vec![-1, -2],
            additional_targets: vec![-2, -1],
            crit_targets: Vec::new(),
            additional_crit_targets: vec![-1],
        },
    ]);

    let mut dispatch_execution = SkillExecution::new(TargetContext::default());
    emit_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut dispatch_execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert_eq!(dispatch_execution.additional_damage_targets, None);

    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1, -2]);
    execution.additional_damage_targets = dispatch_execution.additional_damage_targets;
    for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
        execution.activate_additional_damage(activation.additional);
    }
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: SKILL_ID,
            source_uid: 10,
            target_code: 202,
            targets: vec![-1, -2],
            additional_targets: vec![-2, -1],
            crit_targets: Vec::new(),
            additional_crit_targets: vec![-1],
        },
    ]);
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
        HpCommand::Damage(hit) => Some((hit.target_uid, hit.hurt.is_crit)),
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(hits, vec![(-1, true), (-2, false)]);
}

fn assassination_damage_pair(
    inherent_assassination: bool,
    source_bonus: bool,
    target_trigger: bool,
    deployed_shell: bool,
) -> [(i32, bool); 2] {
    crate::test_support::init_config();
    let mut source_buffs = vec![BuffInfo {
        uid: Some(1),
        buff_id: Some(31260151),
        from_uid: Some(10),
        count: Some(1),
        ..Default::default()
    }];
    if source_bonus {
        source_buffs.push(BuffInfo {
            uid: Some(2),
            buff_id: Some(312451460),
            from_uid: Some(10),
            ..Default::default()
        });
    }
    let mut target_buffs = target_trigger
        .then_some(BuffInfo {
            uid: Some(3),
            buff_id: Some(31240121),
            from_uid: Some(10),
            layer: Some(1),
            ..Default::default()
        })
        .into_iter()
        .collect::<Vec<_>>();
    if deployed_shell {
        target_buffs.push(BuffInfo {
            uid: Some(4),
            buff_id: Some(31090112),
            from_uid: Some(11),
            layer: Some(2),
            ..Default::default()
        });
    }
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        technic: Some(450),
                        ..Default::default()
                    }),
                    buffs: source_buffs,
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
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    technic: Some(120),
                    ..Default::default()
                }),
                buffs: target_buffs,
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
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    if deployed_shell {
        invocation.mode = SkillExecutionMode::Active;
        invocation.card_index = 0;
    }
    let mut execution = SkillExecution::new(TargetContext {
        active_skill_assassinate: inherent_assassination,
        extra_skill_kind: if deployed_shell {
            ExtraSkillKind::Riposte.id()
        } else {
            0
        },
        ..Default::default()
    });
    execution.configured_targets = Some(vec![-1]);
    for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
        execution.activate_additional_damage(activation.additional);
    }

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut RoundDeterminism::default(),
        &mut execution,
    );
    let [HpCommand::Damage(main)] = damage.damage.as_slice() else {
        panic!("expected one main damage command")
    };
    let [HpCommand::Damage(linked)] = damage.additional_damage.as_slice() else {
        panic!("expected one linked damage command")
    };
    [
        (main.amount, main.assassinate),
        (linked.amount, linked.assassinate),
    ]
}

#[test]
fn target_triggered_assassination_converts_main_and_linked_damage() {
    let baseline = assassination_damage_pair(false, false, false, false);
    let converted = assassination_damage_pair(false, false, true, false);

    assert_eq!([converted[0].1, converted[1].1], [true, true]);
    assert!(converted[0].0 > baseline[0].0);
    assert!(converted[1].0 > baseline[1].0);
}

#[test]
fn inherent_assassination_keeps_its_bonus_out_of_linked_damage() {
    let baseline = assassination_damage_pair(true, false, false, false);
    let source_bonus = assassination_damage_pair(true, true, false, false);

    assert_eq!([source_bonus[0].1, source_bonus[1].1], [true, false]);
    assert!(source_bonus[0].0 > baseline[0].0);
    assert_eq!(source_bonus[1].0, baseline[1].0);
}

#[test]
fn target_owned_riposte_damage_bonus_applies_to_main_and_linked_damage() {
    let baseline = assassination_damage_pair(true, false, false, false);
    let boosted = assassination_damage_pair(true, false, false, true);

    assert!(boosted[0].0 > baseline[0].0);
    assert!(boosted[1].0 > baseline[1].0);
    assert!(boosted[0].0 - baseline[0].0 > boosted[1].0 - baseline[1].0);
}

#[test]
fn linked_attack_attributes_keep_temporary_critical_damage_source_owned() {
    let local = vec![
        (AttrId::DmgBonus, 200),
        (AttrId::CriticalRate, 1_000),
        (AttrId::CriticalDmg, 500),
        (AttrId::IncantationMight, 300),
        (AttrId::UltimateIncantationMightMultiplier, 1_000),
    ];
    let shared = vec![
        (AttrId::CriticalResistRate, 300),
        (AttrId::CriticalDmg, -700),
    ];

    assert_eq!(
        plan::LinkedDamagePlan::attack_attributes(&local, &shared, 10, 10),
        vec![
            (AttrId::DmgBonus, 200),
            (AttrId::CriticalRate, 1_000),
            (AttrId::CriticalDmg, 500),
            (AttrId::CriticalResistRate, 300),
            (AttrId::CriticalDmg, -700)
        ]
    );
    assert_eq!(
        plan::LinkedDamagePlan::attack_attributes(&local, &shared, 10, 11),
        vec![
            (AttrId::DmgBonus, 200),
            (AttrId::CriticalResistRate, 300),
            (AttrId::CriticalDmg, -700)
        ]
    );
}

fn hedone_fix_attr_damage(skill_id: i32, target_buff: Option<i32>) -> (i32, i32) {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3145),
                    ex_skill_level: Some(2),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31050144),
                        from_uid: Some(20),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
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
                buffs: target_buff
                    .map(|buff_id| BuffInfo {
                        uid: Some(2),
                        buff_id: Some(buff_id),
                        from_uid: Some(10),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    catalog.insert_damage_rate(skill_id, 5_000);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);
    for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
        execution.activate_additional_damage(activation.additional);
    }
    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        skill_id,
        &mut RoundDeterminism::default(),
        &mut execution,
    );
    let [HpCommand::Damage(main)] = damage.damage.as_slice() else {
        panic!("expected one main damage command")
    };
    let [HpCommand::Damage(linked)] = damage.additional_damage.as_slice() else {
        panic!("expected one linked damage command")
    };
    (main.amount, linked.amount)
}

#[test]
fn hedone_fix_attr_reaches_main_and_linked_conduit_damage() {
    const CONDUIT_SKILL: i32 = 31450113;
    const ORDINARY_SKILL: i32 = 999_999_013;
    let conduit = hedone_fix_attr_damage(CONDUIT_SKILL, None);
    let conduit_scar = hedone_fix_attr_damage(CONDUIT_SKILL, Some(31560122));
    let ordinary = hedone_fix_attr_damage(ORDINARY_SKILL, None);
    let ordinary_scar = hedone_fix_attr_damage(ORDINARY_SKILL, Some(31560122));

    assert_eq!(conduit, (5_000, 300));
    assert_eq!(conduit_scar, (5_400, 324));
    assert_eq!(ordinary, (5_000, 300));
    assert_eq!(ordinary_scar, ordinary);
}

#[test]
fn linked_damage_plan_resolves_committed_credited_source_state() {
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
                        uid: Some(1),
                        buff_id: Some(31050144),
                        from_uid: Some(11),
                        ..Default::default()
                    }],
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
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(434121),
                        from_uid: Some(11),
                        layer: Some(9),
                        ..Default::default()
                    }],
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
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        11,
        &HeroExAttribute {
            cri: Some(0),
            cri_dmg: Some(1_500),
            ..Default::default()
        },
    );
    let pool = TargetPool::from_fight(&fight);
    const SKILL_ID: i32 = 999_999_007;
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);
    execution
        .modifiers
        .attack_attributes
        .push((AttrId::CriticalDmg, 500));
    execution
        .modifiers
        .attack_attributes
        .push((AttrId::CriticalRate, 1_000));
    for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
        execution.activate_additional_damage(activation.additional);
    }

    managers.buff.add_replacing_excluded_with_layer_specified(
        &managers.hp,
        11,
        11,
        434121,
        3,
        true,
    );
    assert_eq!(managers.buff.buff_id_amount(11, 434121), 12);

    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(SKILL_ID, 10, [false]);
    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut determinism,
        &mut execution,
    );
    let [HpCommand::Damage(linked)] = damage.additional_damage.as_slice() else {
        panic!("expected one linked damage command")
    };

    assert_eq!(linked.source_uid, 11);
    assert!(!linked.hurt.is_crit);
    assert_eq!(linked.amount, 300);
}
