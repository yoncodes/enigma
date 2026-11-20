use super::*;

fn configured_damage_ops(
    logic_target: i32,
    behavior_target: i32,
    enemy_uids: &[i64],
    action_target_buff: Option<ActionTargetRoute>,
) -> Vec<RuleOp> {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemy_uids
                .iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(*uid),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    let mut slots = vec![SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(60082, "Redirect"), Vec::new(), Vec::new()),
        TargetRequest {
            code: behavior_target,
            raw: Vec::new(),
        },
    )];
    if let Some(route) = action_target_buff {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![5_051], Vec::new()),
            TargetRequest {
                code: match route {
                    ActionTargetRoute::Direct => 0,
                    ActionTargetRoute::FromCondition => logic_target,
                },
                raw: Vec::new(),
            },
        );
        if matches!(route, ActionTargetRoute::FromCondition) {
            slot.target_from_condition = true;
            slot.condition_target = TargetRequest::self_only();
        }
        slots.push(slot);
    }
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots,
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, logic_target);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(enemy_uids[0]);

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
}

#[test]
fn configured_damage_target_overrides_an_unmapped_logic_target() {
    let ops = configured_damage_ops(230, 1, &[-1], None);

    assert!(
        ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::HpBatch(commands))
                if matches!(
                    commands.as_slice(),
                    [HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                        target_uid: -1,
                        ..
                    })]
                )
        )),
        "{ops:#?}"
    );
}

#[test]
fn predicate_origin_damage_precedes_the_skills_row_damage() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(1_000),
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
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30014, "OriginDamage"),
                vec![0, AttrId::Attack as i32, 100],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let ops = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let origin = ops
        .iter()
        .position(|op| {
            matches!(
                op,
                RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage)))
                    if damage.effect_kind
                        == crate::engine::manager::hp::DamageEffectKind::Genesis
            )
        })
        .expect("predicate origin damage should emit");
    let row = ops
        .iter()
        .position(|op| matches!(op, RuleOp::Command(BattleCommand::HpBatch(_))))
        .expect("row damage should emit");

    assert!(origin < row, "origin={origin} row={row} ops={ops:#?}");
}

#[test]
fn purple_emanation_applies_configured_halo_before_destined_doom_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
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
    let mut managers = BattleManagers::seeded(&fight);
    assert!(managers.emanation.select(10, 110));
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31340151], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31340151,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

    let mut execution = SkillExecution::new(TargetContext::default());
    let mut determinism = RoundDeterminism::default();
    let immediate = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let immediate_completed = immediate
        .ops
        .iter()
        .position(|op| {
            matches!(
                op.op,
                RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(
                        crate::engine::skill::action::SkillActionEvent {
                            phase: SkillPhase::Immediate,
                            ..
                        }
                    )
                )
            )
        })
        .unwrap();
    let halo = immediate
        .ops
        .iter()
        .enumerate()
        .filter_map(|(index, emission)| match &emission.op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if grant.buff_id == 31340001 =>
            {
                Some((index, grant.target_uid, grant.amount))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let freeze = immediate
        .ops
        .iter()
        .position(|emission| matches!(emission.op, RuleOp::FreezeActiveSkillRates))
        .unwrap();

    assert_eq!(freeze, immediate_completed + 1);
    assert_eq!(
        halo,
        vec![(freeze + 1, -1, Some(2)), (freeze + 2, -2, Some(2)),]
    );
    assert!(!immediate.ops.iter().any(|emission| matches!(
        emission.op,
        RuleOp::Command(BattleCommand::Hp(_) | BattleCommand::HpBatch(_))
    )));

    let continuation = immediate.continuation.unwrap();
    assert_eq!(continuation.phase, Some(SkillPhase::Damage));
    let damage = emit_ops(
        continuation,
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();
    assert!(damage.ops.iter().any(|emission| matches!(
        emission.op,
        RuleOp::Command(BattleCommand::Hp(_) | BattleCommand::HpBatch(_))
    )));
    assert!(!damage.ops.iter().any(|emission| matches!(
        &emission.op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
            if grant.buff_id == 31340001
    )));
}

#[test]
fn action_target_damage_routing_keeps_configured_mass_targets() {
    let ops = configured_damage_ops(202, 0, &[-1, -2], None);

    assert!(
        ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::HpBatch(commands))
                if matches!(
                    commands.as_slice(),
                    [
                        HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                            target_uid: -1,
                            ..
                        }),
                        HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                            target_uid: -2,
                            ..
                        })
                    ]
                )
        )),
        "{ops:#?}"
    );
}

#[test]
fn configured_damage_routing_preserves_other_action_target_behaviors() {
    let ops = configured_damage_ops(1, 1, &[-1], Some(ActionTargetRoute::Direct));

    assert!(
        ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: -1,
                buff_id: 5_051,
                ..
            })))
        )),
        "{ops:#?}"
    );
}

#[test]
fn configured_damage_routing_preserves_condition_derived_action_targets() {
    let ops = configured_damage_ops(1, 1, &[-1], Some(ActionTargetRoute::FromCondition));

    assert!(
        ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: -1,
                buff_id: 5_051,
                ..
            })))
        )),
        "{ops:#?}"
    );
}

#[test]
fn configured_ignore_beat_back_reaches_the_committed_hit() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
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
                buffs: vec![BuffInfo {
                    uid: Some(-20),
                    buff_id: Some(30940182),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
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
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.set_shield(-1, 100);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60054, "IgnoreBeatBack"),
                Vec::new(),
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let ops = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let commands = ops
        .into_iter()
        .find_map(|op| match op {
            RuleOp::Command(BattleCommand::HpBatch(commands)) => Some(commands),
            _ => None,
        })
        .expect("configured attack should emit damage");
    assert!(matches!(
        commands.as_slice(),
        [HpCommand::Damage(crate::engine::manager::hp::HpDamage {
            ignore_riposte: true,
            ..
        })]
    ));

    let mut ordinary = managers.clone();
    let mut counterable = commands.clone();
    let [HpCommand::Damage(hit)] = counterable.as_mut_slice() else {
        unreachable!()
    };
    hit.ignore_riposte = false;
    let earned = ordinary.execute_rule_hp_batch(counterable).unwrap();
    assert_eq!(earned[0].shield_counter_credits.len(), 1);

    let committed = managers.execute_rule_hp_batch(commands).unwrap();
    assert!(committed[0].shield_counter_credits.is_empty());
    let events = committed
        .into_iter()
        .flat_map(|hit| hit.changes.events())
        .collect::<Vec<_>>();
    let hit_event = events
        .into_iter()
        .find(|event| matches!(event, BattleEvent::Hit(_)))
        .expect("committed damage should publish a hit");
    let BattleEvent::Hit(hit) = hit_event else {
        unreachable!()
    };
    assert!(hit.ignore_riposte);
    assert_eq!(hit.shield_absorbed, 100);
    let kinds = BattleEvent::Hit(hit)
        .subscription_kinds()
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            EventKind::TargetAttacked,
            EventKind::BeAttacked,
            EventKind::Riposte,
        ]
    );
}

#[test]
fn row_damage_applies_configured_excess_crit_conversion() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31280114),
                        layer: Some(4),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(3),
                        buff_id: Some(31280112),
                        duration: Some(2),
                        from_uid: Some(10),
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
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        11,
        &HeroExAttribute {
            cri: Some(1_500),
            cri_dmg: Some(1_000),
            ..Default::default()
        },
    );
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_damage_rate(100, 1_000);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 11,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(100, 11, [true]);
    let mut execution = SkillExecution::new(TargetContext::default());

    let ops = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        100,
        &mut determinism,
        &mut execution,
    )
    .unwrap();

    assert!(matches!(
        ops.damage.as_slice(),
        [HpCommand::Damage(crate::engine::manager::hp::HpDamage {
            amount: 1_800,
            ..
        })]
    ));
}

#[test]
fn excess_crit_conversion_keeps_the_fractional_remainder() {
    assert_eq!(
        super::super::super::plan::split_excess_crit_conversion(66, 750),
        (49, 500)
    );
}

#[test]
fn row_damage_consumes_captured_crit_choices_in_target_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 201);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1, -2]);
    execution.context.additional_skill_target_count = 1;
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(100, 10, [false, true, false]);

    let ops = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        100,
        &mut determinism,
        &mut execution,
    )
    .unwrap();
    assert!(ops.additional_damage.is_empty());
    assert_eq!(execution.configured_targets, Some(vec![-1, -2, -3]));
    let kinds = ops
        .damage
        .into_iter()
        .filter_map(|op| match op {
            HpCommand::Damage(damage) => Some(damage.effect_kind),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            DamageEffectKind::Normal,
            DamageEffectKind::Critical,
            DamageEffectKind::Normal,
        ]
    );
    assert_eq!(execution.context.action_crit_count, 1);
}

#[test]
fn additional_damage_activation_survives_its_pre_damage_resource_cost() {
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
                    buff_id: Some(31050145),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(500),
                    attr: Some(HeroAttribute {
                        hp: Some(500),
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
    let mut managers = BattleManagers::seeded(&fight);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext {
        extra_skill_kind: 1,
        ..Default::default()
    });

    let mut activations = plan::LinkedDamagePlan::activations(&invocation, &managers, &execution);
    let activation = activations.pop().unwrap();
    let [RuleOp::Command(BattleCommand::Eureka(command))] = activation.buff_act_ops.as_slice()
    else {
        panic!("expected the configured Eureka cost")
    };
    assert!(activation.skill_ops.is_empty());
    assert_eq!(activation.temporary_buff, None);
    execution.activate_additional_damage(activation.additional);
    managers.execute_eureka(command.clone()).unwrap();
    let fallback = plan::LinkedDamagePlan::activations(&invocation, &managers, &execution);
    assert_eq!(fallback.len(), 1);
    assert!(fallback[0].buff_act_ops.is_empty());

    execution.configured_targets = Some(vec![-1]);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert_logic_target(100, 1);
    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        100,
        &mut RoundDeterminism::default(),
        &mut execution,
    )
    .unwrap();
    assert!(!damage.additional_damage.is_empty());

    execution.pending_additional_damage = damage.additional_damage;
    execution.pending_additional_damage[0]
        .commands
        .push(HpCommand::Lose(crate::engine::manager::hp::HpLoss {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100, "AbsorbedDamage"),
            },
            source_uid: 10,
            target_uid: -3,
            amount: 50,
            config_effect: -1,
            hurt: None,
        }));
    assert!(managers.hp.current(-3) > 0);
    managers
        .execute_hp(HpCommand::Kill(HpKill {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100, "PrimaryDamage"),
            },
            source_uid: 10,
            target_uid: -1,
            config_effect: 1,
        }))
        .unwrap();
    assert!(
        managers
            .execute_linked_damage(execution.take_additional_damage())
            .unwrap()
            .is_empty()
    );
}
