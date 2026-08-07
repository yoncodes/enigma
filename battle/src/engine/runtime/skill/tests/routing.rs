use super::*;

#[test]
fn compiled_slot_emits_its_registered_destination_op() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
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
                BehaviorSpec::new(60189, "AddEnergyToCard"),
                vec![1, 2, 3],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        )],
    });

    assert!(matches!(
        emit_all_ops(
            SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into(),
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Active,
        ),
        Ok(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::ChangeBasicEnergy(CardEnergyChange {
                    delta: 2,
                    count: 3,
                    ..
                })
            ))]
        )
    ));
}

#[test]
fn configured_target_rule_owns_slots_that_use_logic_target() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_point: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_point: Some(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(80),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(20),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
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
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_logic_target(100, 112);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    assert!(matches!(
        emit_all_ops(
            invocation.clone(),
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Active,
        ),
        Ok(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(change)
            ))] if change.target_uid == 11
        )
    ));

    invocation.target = SkillTarget::LogicRule(210);
    assert!(matches!(
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Active,
        ),
        Ok(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(change)
            ))] if change.target_uid == -2
        )
    ));
}

#[test]
fn behavior_target_from_logic_target_keeps_the_runtime_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11, 12, 13]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(31_080_131).unwrap();
    assert_eq!(effect.slots[3].behavior.arg(0), Some(31_080_131));
    assert_eq!(effect.slots[3].target.code, 103);
    assert_eq!(effect.slots[4].behavior.arg(0), Some(31_080_132));
    assert_eq!(effect.slots[4].target.code, 1);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31_080_131,
    }
    .into();
    invocation.target = SkillTarget::Explicit(11);

    let grants = emit_all_ops(
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
    .filter_map(|op| match op {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
            if matches!(grant.buff_id, 31_080_131 | 31_080_132) =>
        {
            Some((grant.buff_id, grant.target_uid))
        }
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(grants, vec![(31_080_131, 10), (31_080_132, 11)]);
}

#[test]
fn ritual_dance_threshold_updates_the_current_action_before_lifecycle_publication() {
    crate::test_support::init_config();

    let action_kind = |layer| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(1_001),
                        buff_id: Some(31100201),
                        from_uid: Some(10),
                        layer: Some(layer),
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
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        defense: Some(100),
                        mdefense: Some(100),
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
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 31100531,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        let mut execution = SkillExecution::new(TargetContext::default());
        let emission = emit_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();
        let lifecycle_kind = emission.ops.iter().find_map(|emission| match &emission.op {
            RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action),
            ) if action.phase == SkillPhase::Immediate => Some(action.extra_skill_kind),
            _ => None,
        });

        (execution.context.extra_skill_kind, lifecycle_kind)
    };

    assert_eq!(action_kind(3), (0, Some(0)));
    assert_eq!(
        action_kind(4),
        (
            crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction.id(),
            Some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction.id()),
        )
    );
}

#[test]
fn buff_count_multiplier_runs_after_damage_for_every_action_target() {
    crate::test_support::init_config();
    const CHIRP_SIGNAL: i32 = 116_385_671;
    const ECHOES_OF_BYGONE_DAYS: i32 = 116_385_672;
    const SKILL_ID: i32 = 116_385_665;

    let targets = [
        (10, 4, 1090),
        (11, 4, 1091),
        (12, 3, 1092),
        (13, 2, 1093),
        (14, 8, 1094),
    ];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: targets
                .iter()
                .map(|&(uid, layer, buff_uid)| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100_000),
                    attr: Some(HeroAttribute {
                        hp: Some(100_000),
                        defense: Some(1_000),
                        mdefense: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(buff_uid),
                        buff_id: Some(CHIRP_SIGNAL),
                        from_uid: Some(-1),
                        layer: Some(layer),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(2090),
                    buff_id: Some(ECHOES_OF_BYGONE_DAYS),
                    from_uid: Some(-1),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: SKILL_ID,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    let mut execution = SkillExecution::new(TargetContext::default());
    let commands = loop {
        let phase = invocation.phase.unwrap_or(SkillPhase::Immediate);
        let emission = emit_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();
        let phase_commands = emission
            .ops
            .into_iter()
            .filter_map(|emission| match emission.op {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Accumulate(grant)))
                    if grant.origin.key.matches(60022, "BuffCountMulti") =>
                {
                    Some(BuffCommand::Accumulate(grant))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if phase == SkillPhase::AfterDamage {
            break phase_commands;
        }
        assert!(phase_commands.is_empty());
        invocation = emission.continuation.expect("skill reached AfterDamage");
    };

    let command_targets = commands
        .iter()
        .map(|command| match command {
            BuffCommand::Accumulate(grant) => grant.target_uid,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        command_targets,
        vec![
            10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 13, 13, 14, 14, 14, 14, 14, 14, 14, 14,
        ]
    );

    let mut capped_refreshes = Vec::new();
    let mut capped_noops = 0;
    for command in commands {
        let target_uid = match &command {
            BuffCommand::Accumulate(grant) => grant.target_uid,
            _ => unreachable!(),
        };
        let uid = managers.buff.buff_id_uid(target_uid, CHIRP_SIGNAL);
        let changes = managers.execute_buff(command).unwrap();
        if target_uid == 14 {
            if let Some(refresh) = changes.change.refreshed.first() {
                capped_refreshes.push((
                    refresh.before.layer.unwrap_or_default(),
                    refresh.after.layer.unwrap_or_default(),
                ));
            } else {
                capped_noops += 1;
            }
        }
        assert_eq!(managers.buff.buff_id_uid(target_uid, CHIRP_SIGNAL), uid);
    }
    for &(uid, layer, _) in &targets {
        assert_eq!(
            managers.buff.buff_id_amount(uid, CHIRP_SIGNAL),
            (layer * 2).min(10)
        );
    }
    assert_eq!(capped_refreshes, vec![(8, 9), (9, 10)]);
    assert_eq!(capped_noops, 6);
}

#[test]
fn mass_status_dispel_routes_to_every_enemy_and_commits_through_the_buff_manager() {
    crate::test_support::init_config();
    let target = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        buffs: vec![
            BuffInfo {
                uid: Some(uid * 10),
                buff_id: Some(530000111),
                ..Default::default()
            },
            BuffInfo {
                uid: Some(uid * 10 + 1),
                buff_id: Some(530000112),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![target(10), target(11)],
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
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let commands = emit_all_ops(
        SkillRequest {
            source_uid: -1,
            skill_id: 1163855015,
        }
        .into(),
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap()
    .into_iter()
    .filter_map(|op| match op {
        RuleOp::Command(BattleCommand::Buff(command)) => match &command {
            BuffCommand::Dispel(dispel) if dispel.origin.key.matches(30003, "Disperse1") => {
                Some(command)
            }
            _ => None,
        },
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(
        commands
            .iter()
            .filter_map(|command| match command {
                BuffCommand::Dispel(dispel) => Some(dispel.target_uid),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
    for command in commands {
        managers.execute_buff(command).unwrap();
    }
    for uid in [10, 11] {
        assert!(!managers.buff.has_buff_id(uid, 530000111));
        assert!(managers.buff.has_buff_id(uid, 530000112));
    }
}

#[test]
fn target_career_selects_every_matching_plant_ally() {
    crate::test_support::init_config();
    let entity = |uid, model_id, career, position| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        career: Some(career),
        position: Some(position),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3065, 3, 1),
                entity(11, 3003, 3, 2),
                entity(12, 3087, 3, 3),
                entity(13, 3095, 5, 4),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 11,
        skill_id: 30_030_210,
    }
    .into();
    invocation.target = SkillTarget::Explicit(11);

    let grants = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            active_skill_slot: 0,
            ..Default::default()
        },
        &SkillOpTrigger::Active,
    )
    .unwrap()
    .into_iter()
    .filter_map(|op| match op {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
            if grant.buff_id == 30_030_207 =>
        {
            Some((grant.buff_id, grant.target_uid))
        }
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(
        grants,
        vec![(30_030_207, 10), (30_030_207, 11), (30_030_207, 12)]
    );
    assert!(grants.iter().all(|(_, target_uid)| *target_uid != 13));
}

#[test]
fn enhanced_circle_variants_preserve_each_target_rules_order() {
    crate::test_support::init_config();
    let entity = |uid, model_id, career, position| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        career: Some(career),
        position: Some(position),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3065, 3, 1),
                entity(11, 3003, 3, 2),
                entity(12, 3087, 3, 3),
                entity(13, 3095, 5, 4),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 11,
        skill_id: 30_030_211,
    }
    .into();
    invocation.target = SkillTarget::Explicit(11);

    let grants = emit_all_ops(
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
    .filter_map(|op| match op {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
            if matches!(grant.buff_id, 30_030_207..=30_030_209) =>
        {
            Some((grant.buff_id, grant.target_uid))
        }
        _ => None,
    })
    .collect::<Vec<_>>();

    assert_eq!(
        grants,
        vec![
            (30_030_208, 10),
            (30_030_209, 12),
            (30_030_207, 10),
            (30_030_207, 11),
            (30_030_207, 12),
        ]
    );
}

#[test]
fn cast_local_modifier_flows_into_later_origin_damage() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),

                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1000),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
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
            cri_dmg: Some(1000),
            ..Default::default()
        },
    );
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(10004, "AttrFix"),
                    vec![AttrId::CriticalDmg as i32, 500],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30015, "OriginDamageCanCrit"),
                    vec![0, AttrId::CurrentHp as i32, 1000],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
        ],
    });
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(100, 10, [true]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    assert!(matches!(
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            &SkillOpTrigger::Active,
        ),
        Ok(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                amount: 150,
                ..
            })))]
        )
    ));
}
