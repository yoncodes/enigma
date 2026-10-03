use super::*;

enum ActionTargetRoute {
    Direct,
    FromCondition,
}

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

    let events = managers
        .execute_hp_batch(commands)
        .unwrap()
        .into_iter()
        .flat_map(|changes| changes.events())
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

    let subscriber = crate::engine::skill::subscriber::BuffActSubscriber {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: -20,
        buff_id: 1,
        team_type: 2,
        owner_alive: true,
        amount: 1,
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::Riposte,
            DefinitionKey::new(802, "BeatBackByCounter"),
        ),
        act_type: "BeatBackByCounter".to_owned(),
        effect_time: 401,
        effect_condition: 0,
        args: vec![200],
        raw: "802#200".to_owned(),
    };
    assert!(
        crate::engine::skill::buff_act::riposte::shielded_ally_rule_ops(
            &pool,
            &subscriber,
            &BattleEvent::Hit(hit),
        )
        .unwrap()
        .is_empty()
    );
    let mut counterable = hit;
    counterable.ignore_riposte = false;
    assert!(matches!(
        crate::engine::skill::buff_act::riposte::shielded_ally_rule_ops(
            &pool,
            &subscriber,
            &BattleEvent::Hit(counterable),
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Skill(_)]
    ));
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
    );

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
        super::super::plan::split_excess_crit_conversion(66, 750),
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
    );
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

    let mut activations = plan::additional_damage_activation(&invocation, &managers, &execution);
    let activation = activations.pop().unwrap();
    let [RuleOp::Command(BattleCommand::Eureka(command))] = activation.buff_act_ops.as_slice()
    else {
        panic!("expected the configured Eureka cost")
    };
    assert!(activation.skill_ops.is_empty());
    assert_eq!(activation.temporary_buff, None);
    execution
        .activated_additional_damage
        .push(activation.additional);
    managers.execute_eureka(command.clone()).unwrap();
    assert!(plan::additional_damage_activation(&invocation, &managers, &execution).is_empty());

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
    );
    assert!(!damage.additional_damage.is_empty());

    execution.pending_additional_damage = damage.additional_damage;
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
    assert!(execution.take_live_additional_damage(&managers).is_empty());
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
    let mut catalog = SkillEffectCatalog::default();
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
        execution.activated_additional_damage.extend(
            plan::additional_damage_activation(&invocation, &managers, &execution)
                .into_iter()
                .map(|activation| activation.additional),
        );
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
fn additional_damage_keeps_its_own_target_order_and_critical_targets() {
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
    let mut catalog = SkillEffectCatalog::default();
    const SKILL_ID: i32 = 999_999_004;
    catalog.insert_damage_rate(SKILL_ID, 1_000);
    let invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL_ID,
    }
    .into();
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1, -2]);
    execution.configured_additional_targets = Some(vec![-2, -1]);
    execution.activated_additional_damage.extend(
        plan::additional_damage_activation(&invocation, &managers, &execution)
            .into_iter()
            .map(|activation| activation.additional),
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: SKILL_ID,
            source_uid: 10,
            target_code: -1,
            targets: vec![-1, -2],
            additional_targets: vec![-2, -1],
            crit_targets: Vec::new(),
            additional_crit_targets: vec![-1],
        },
    ]);

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut determinism,
        &mut execution,
    );
    let hits = damage
        .additional_damage
        .into_iter()
        .filter_map(|command| match command {
            HpCommand::Damage(hit) => Some((hit.target_uid, hit.hurt.is_crit)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(hits, vec![(-2, false), (-1, true)]);
}

#[test]
fn configured_and_active_additional_damage_producers_both_resolve() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(20_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31260151),
                        from_uid: Some(10),
                        count: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31260171),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(20_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    const SKILL_ID: i32 = 999_999_005;
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
        .additional_damage
        .push(AdditionalDamageModifier {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60206, "CreateAdditionalDamageAddBuff"),
            },
            buff_id: 31200113,
        });
    let activations = plan::additional_damage_activation(&invocation, &managers, &execution);
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].additional.feature.buff_id, 31260151);
    execution.activated_additional_damage.extend(
        activations
            .into_iter()
            .map(|activation| activation.additional),
    );
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60206, "CreateAdditionalDamageAddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31200113,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL_ID,
        &mut RoundDeterminism::default(),
        &mut execution,
    );
    let amounts = damage
        .additional_damage
        .into_iter()
        .filter_map(|command| match command {
            HpCommand::Damage(hit) => Some(hit.amount),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(amounts, vec![5_000, 3_000]);
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
            crate::engine::skill::condition::extra::ExtraSkillKind::Riposte.id()
        } else {
            0
        },
        ..Default::default()
    });
    execution.configured_targets = Some(vec![-1]);
    execution.activated_additional_damage.extend(
        plan::additional_damage_activation(&invocation, &managers, &execution)
            .into_iter()
            .map(|activation| activation.additional),
    );

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
fn target_owned_extra_action_bonuses_apply_to_main_and_linked_damage() {
    let baseline = assassination_damage_pair(true, false, false, false);
    let boosted = assassination_damage_pair(true, false, false, true);

    assert!(boosted[0].0 > baseline[0].0);
    assert!(boosted[1].0 > baseline[1].0);
}

#[test]
fn before_crit_modifier_uses_the_same_planned_outcome_as_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    technic: Some(100),
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
                    defense: Some(500),
                    mdefense: Some(500),
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
    let catalog = crate::engine::skill::effect::catalog::global();

    for (is_crit, expects_penetration) in [(true, true), (false, false)] {
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(40120521, 10, [is_crit]);
        let mut execution = SkillExecution::new(TargetContext::default());
        let immediate = emit_ops(
            SkillRequest {
                source_uid: 10,
                skill_id: 40120521,
            }
            .into(),
            &managers,
            &pool,
            catalog,
            &mut determinism,
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();
        let damage = immediate.continuation.unwrap();

        let emission = emit_ops(
            damage,
            &managers,
            &pool,
            catalog,
            &mut determinism,
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();

        assert_eq!(execution.planned_crits, Some(vec![(-1, is_crit)]));
        assert_eq!(execution.context.action_crit_count, i32::from(is_crit));
        assert_eq!(
            execution
                .modifiers
                .attack_attributes
                .contains(&(AttrId::Penetration, 500)),
            expects_penetration
        );
        assert!(
            emission
                .ops
                .iter()
                .any(|output| matches!(output.op, RuleOp::Command(BattleCommand::HpBatch(_))))
        );
    }
}

#[test]
fn exact_attack_crit_row_gains_moxie_only_after_a_critical_hit() {
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(40120512, 10, [true]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 40120512,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 1);
}

#[test]
fn bloodlust_applies_leech_efficacy_modifiers_to_committed_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
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
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_sp(
        10,
        &sonettobuf::HeroSpAttribute {
            absorb: Some(100),
            ..Default::default()
        },
    );
    managers.buff.add(&managers.hp, 10, 10, 90120022, 0);
    let target_before = managers.hp.current(-1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 40120621,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let dealt = target_before - managers.hp.current(-1);
    assert!(dealt > 0);
    assert_eq!(
        managers.hp.current(10),
        100 + crate::engine::damage::scale_permille(
            crate::engine::damage::scale_permille(dealt, 300),
            700,
        )
    );
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::Hp(execution)
            if execution.changes.hp.is_some_and(|hp| {
                hp.effect_type == sonettobuf::effect_type_enum::EffectType::Bloodlust as i32
            })
    )));
}

#[test]
fn mei_lei_er_extra_round_attack_forces_afflatus_advantage() {
    crate::test_support::init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        model_id: (uid == 10).then_some(3146),
        team_type: Some(team_type),
        career: Some(1),
        current_hp: Some(100_000),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31460183,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(result.outcomes.iter().any(|outcome| {
        match outcome {
            crate::engine::runtime::executor::RuleOutcome::Hp(execution) => execution
                .changes
                .damage
                .is_some_and(|damage| damage.hurt.career_restraint),
            crate::engine::runtime::executor::RuleOutcome::HpBatch(batch) => {
                batch.iter().any(|execution| {
                    execution
                        .changes
                        .damage
                        .is_some_and(|damage| damage.hurt.career_restraint)
                })
            }
            _ => false,
        }
    }));
}

#[test]
fn dodged_attack_does_not_apply_effects_to_the_target_hit() {
    crate::test_support::init_config();
    let attacker =
        crate::engine::fight::defender::Defender::build_monster_with_uid(100109, 10, 1, 1).unwrap();
    let mut defender =
        crate::engine::fight::defender::Defender::build_monster_with_uid(100108, -1, 1, 2).unwrap();
    defender.buffs.push(BuffInfo {
        uid: Some(1002),
        buff_id: Some(22181),
        duration: Some(2),
        from_uid: Some(-1),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![attacker],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![defender],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let hp_before = managers.hp.current(-1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30230112,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

    crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), hp_before);
    assert!(!managers.buff.has_buff_id(-1, 4051));
}

#[test]
fn an_action_not_played_from_a_card_pays_the_extra_action_eureka_cost() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
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
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let execution = SkillExecution::new(TargetContext::default());
    let pays_eureka = |card_index| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into();
        invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
        invocation.card_index = card_index;
        plan::additional_damage_activation(&invocation, &managers, &execution)
            .iter()
            .any(|activation| {
                matches!(
                    activation.buff_act_ops.as_slice(),
                    [RuleOp::Command(BattleCommand::Eureka(_))]
                )
            })
    };

    // Paper Parade: "When an ally performs an extra action: Consume 2 of Flutterpage's Eureka".
    assert!(pays_eureka(0));
    assert!(!pays_eureka(1));
}

#[test]
fn an_action_not_played_from_a_card_is_forced_critical_by_the_gust_force_field() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
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
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451011], []);
    let planned_crits = |card_index| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 312451011,
        }
        .into();
        invocation.mode = SkillExecutionMode::Active;
        invocation.card_index = card_index;
        let mut execution = SkillExecution::new(TargetContext::default());
        execution.configured_targets = Some(vec![-1]);
        plan::plan_crits(
            &invocation,
            &managers,
            &pool,
            &catalog,
            312451011,
            &mut RoundDeterminism::default(),
            &mut execution,
        );
        execution.planned_crits
    };

    // "When an ally performs an extra action: ... this action must crit."
    assert_eq!(planned_crits(0), Some(vec![(-1, true)]));
    assert_eq!(planned_crits(1), Some(vec![(-1, false)]));
}

#[test]
fn an_assassination_adds_the_holders_force_field_damage_only_on_marked_targets() {
    crate::test_support::init_config();
    let additional_damage = |marked: bool| {
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
                        uid: Some(20),
                        buff_id: Some(312451456),
                        from_uid: Some(11),
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
                    buffs: marked
                        .then(|| BuffInfo {
                            uid: Some(30),
                            buff_id: Some(31240121),
                            from_uid: Some(11),
                            layer: Some(1),
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
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_damage_rate(100, 6_000);
        catalog.insert_logic_target(100, 1);
        let invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into();
        let mut execution = SkillExecution::new(TargetContext::default());
        execution.configured_targets = Some(vec![-1]);
        for activation in plan::additional_damage_activation(&invocation, &managers, &execution) {
            execution
                .activated_additional_damage
                .push(activation.additional);
        }
        let ops = plan::damage_ops(
            &invocation,
            &managers,
            &pool,
            &catalog,
            100,
            &mut RoundDeterminism::default(),
            &mut execution,
        );
        (ops.damage, ops.additional_damage)
    };

    // Way of Ares: "When an allied [Assassination] is triggered, deal an additional 125% DMG of
    // the attacker's type."
    let (main, marked) = additional_damage(true);
    assert!(matches!(
        marked.as_slice(),
        [HpCommand::Damage(damage)] if damage.source_uid == 10
    ));
    let main_amount = match main.as_slice() {
        [HpCommand::Damage(damage)] => damage.amount,
        _ => panic!("expected one main damage command"),
    };
    let linked_amount = match marked.as_slice() {
        [HpCommand::Damage(damage)] => damage.amount,
        _ => panic!("expected one linked damage command"),
    };
    assert_eq!(linked_amount, main_amount * 1_250 / 6_000);
    assert!(additional_damage(false).1.is_empty());
}

#[test]
fn proportional_assassination_damage_precedes_independently_rolled_linked_damage() {
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
    execution.activated_additional_damage.extend(
        plan::additional_damage_activation(&invocation, &managers, &execution)
            .into_iter()
            .map(|activation| activation.additional),
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(SKILL_ID, 10, [false]);
    determinism.enqueue_additional_crits(SKILL_ID, 10, -1, [true, false]);

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
        HpCommand::Damage(hit) => Some((hit.source_uid, hit.hurt.is_crit)),
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(hits, vec![(10, false), (12, false)]);
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451011], []);
    let damage_phase = |card_index| {
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
                return (ops, execution.pending_additional_damage);
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
    let (ops, additional) = damage_phase(0);
    assert!(opens_with_cost_then_marker(&ops));
    assert!(!additional.is_empty());
    assert!(additional.iter().all(|command| matches!(
        command,
        HpCommand::Damage(damage)
            if damage.effect_kind == crate::engine::manager::hp::DamageEffectKind::Normal
    )));
    let (ops, _) = damage_phase(1);
    assert!(
        !ops.iter()
            .any(|emission| matches!(emission.op, RuleOp::EffectMarker { .. }))
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
    let mut execution = SkillExecution::new(TargetContext::default());
    let ops = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap()
    .ops;
    let position =
        |matches: &dyn Fn(&RuleOp) -> bool| ops.iter().position(|emission| matches(&emission.op));
    let started = position(&|op| {
        matches!(
            op,
            RuleOp::Publish(crate::engine::event::payload::BattleEvent::SkillEffectStarted(_))
        )
    });
    let cost = position(&|op| matches!(op, RuleOp::Command(BattleCommand::Eureka(_))));

    // Psychube 433911 "When the carrier performs an extra action" resolves as the action starts.
    assert!(matches!((started, cost), (Some(started), Some(cost)) if started < cost));
}
