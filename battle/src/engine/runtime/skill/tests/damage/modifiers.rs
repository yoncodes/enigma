use super::*;
use crate::engine::skill::condition::extra::ExtraSkillKind;

#[test]
fn melania_ultimate_critical_attributes_apply_per_layer_only_to_her_ultimate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(10), model_id: Some(3062), career: Some(4), team_type: Some(1),
            current_hp: Some(100_000), attr: Some(HeroAttribute {
                hp: Some(100_000), attack: Some(1_000), ..Default::default()
            }), buffs: vec![BuffInfo { uid: Some(11), buff_id: Some(30620121), from_uid: Some(10),
                layer: Some(2), count: Some(1), ..Default::default() }], ..Default::default()
        }, FightEntityInfo { uid: Some(20), model_id: Some(3079), team_type: Some(1),
            current_hp: Some(100_000), ..Default::default() }], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(-1), career: Some(4), team_type: Some(2), current_hp: Some(100_000),
            attr: Some(HeroAttribute { hp: Some(100_000), ..Default::default() }), ..Default::default()
        }], ..Default::default() }), ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let features = managers.buff.active_features(&managers.hp);
    let entries = features.iter().filter(|feature| feature.owner_uid == 10
        && feature.buff_id == 30620121
        && matches!(crate::engine::skill::buff_act::feature_kind(feature),
            Some(crate::engine::skill::buff_act::registry::BuffActKind::UltimateAttr)))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "both ultimate critical attributes must be registered");
    let delta = |attr, ultimate| entries.iter().map(|feature|
        crate::engine::skill::buff_act::attack_attribute_delta_for_skill(feature, attr,
            &managers.buff, &managers.hp, ultimate, false)).sum::<i32>();
    assert_eq!(delta(AttrId::CriticalRate, true), 120);
    assert_eq!(delta(AttrId::CriticalDmg, true), 120);
    assert_eq!(delta(AttrId::CriticalRate, false), 0);
    assert_eq!(delta(AttrId::CriticalDmg, false), 0);

    let execution = SkillExecution::new(TargetContext::default());
    let ultimate = plan::planned_crit_chance(30620131, 10, -1, &managers, &pool, &execution, false);
    let normal = plan::planned_crit_chance(30620111, 10, -1, &managers, &pool, &execution, false);
    assert_eq!(ultimate - normal, 120);
    let other = plan::planned_crit_chance(30620131, 20, -1, &managers, &pool, &execution, false);
    assert_eq!(other, crate::engine::damage::handler::crit_chance(20, -1, &pool, &managers));

    // Compare forced critical and noncritical hits to isolate the critical-damage
    // portion from the same buff's separately configured ultimate-might bonus.
    let hit = |skill_id: i32, buff_id: i32, critical: bool| {
        let mut scenario = fight.clone();
        if buff_id == 0 {
            scenario.attacker.as_mut().unwrap().entitys[0].buffs.clear();
        } else {
            let buff = &mut scenario.attacker.as_mut().unwrap().entitys[0].buffs[0];
            buff.buff_id = Some(buff_id);
            buff.layer = Some(if buff_id == 30620121 { 2 } else { 1 });
        }
        let mut managers = BattleManagers::seeded(&scenario);
        managers.attribute.override_ex(10, &HeroExAttribute { cri_dmg: Some(1_500), ..Default::default() });
        let pool = TargetPool::from_fight(&scenario);
        let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        catalog.insert_damage_rate(skill_id, 1000);
        let mut invocation: SkillInvocation = SkillRequest { source_uid: 10, skill_id }.into();
        invocation.target = SkillTarget::Explicit(-1);
        let mut execution = SkillExecution::new(TargetContext::default());
        execution.configured_targets = Some(vec![-1]);
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(skill_id, 10, [critical]);
        let damage = plan::damage_ops(&invocation, &managers, &pool, &catalog, skill_id,
            &mut determinism, &mut execution);
        let [HpCommand::Damage(hit)] = damage.damage.as_slice() else { panic!("missing hit"); };
        hit.amount
    };
    let (u0, u1) = (hit(30620131, 0, false), hit(30620131, 30620121, false));
    let (c0, c1) = (hit(30620131, 0, true), hit(30620131, 30620121, true));
    let expected = i64::from(u0) * i64::from(u1) * 120 / 1000;
    let actual = i64::from(c1) * i64::from(u0) - i64::from(c0) * i64::from(u1);
    assert!((actual - expected).abs() <= i64::from(u0 + u1) * 2,
        "ultimate critical damage: noncritical={u0}/{u1}, critical={c0}/{c1}");
    assert_eq!(hit(30620111, 0, true), hit(30620111, 30620121, true),
        "ultimate-only attributes must not affect normal incantations");
    let final_bonus = hit(30620131, 12841051, false);
    assert!((final_bonus * 10 - u0 * 11).abs() <= 10,
        "ultimate final-damage bonus: {u0} -> {final_bonus}");
    assert_eq!(hit(30620111, 0, true), hit(30620111, 12841051, true),
        "ultimate-only final damage must not affect normal incantations");
}

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
        (AttrId::CriticalDmg, 500),
        (AttrId::IncantationMight, 300),
    ];
    let shared = vec![(AttrId::CriticalDmg, -700)];

    assert_eq!(
        plan::LinkedDamagePlan::attack_attributes(&local, &shared, 10, 10),
        vec![
            (AttrId::DmgBonus, 200),
            (AttrId::CriticalDmg, 500),
            (AttrId::CriticalDmg, -700)
        ]
    );
    assert_eq!(
        plan::LinkedDamagePlan::attack_attributes(&local, &shared, 10, 11),
        vec![(AttrId::DmgBonus, 200), (AttrId::CriticalDmg, -700)]
    );
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
            cri: Some(1_500),
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
    determinism.enqueue_additional_crits(SKILL_ID, 10, -1, [true]);
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
    assert_eq!(linked.amount, 493);
}
