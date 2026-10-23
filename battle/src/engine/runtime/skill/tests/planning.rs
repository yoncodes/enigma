use super::*;

fn field_event(kind: crate::engine::manager::field::FieldChangeKind) -> BattleEvent {
    BattleEvent::FieldChanged(crate::engine::event::payload::FieldChangeEvent {
        origin: FIELD_ORIGIN,
        team: 1,
        kind,
        field_id: 30002,
        before_level: 1,
        after_level: 2,
        before_progress: 80,
        after_progress: 100,
        overflow: 0,
    })
}

#[test]
fn field_level_activates_added_field_conditions_but_progress_does_not() {
    let mut level_context = TargetContext::default();
    apply_event_context(
        &BattleManagers::seeded(&Fight::default()),
        &SkillEffectCatalog::default(),
        &mut level_context,
        &field_event(crate::engine::manager::field::FieldChangeKind::Level),
    );
    assert_eq!(level_context.magic_circle_id, 30002);
    assert_eq!(level_context.added_magic_circle_id, 30002);

    let mut progress_context = TargetContext::default();
    apply_event_context(
        &BattleManagers::seeded(&Fight::default()),
        &SkillEffectCatalog::default(),
        &mut progress_context,
        &field_event(crate::engine::manager::field::FieldChangeKind::Progress),
    );
    assert_eq!(progress_context.magic_circle_id, 30002);
    assert_eq!(progress_context.added_magic_circle_id, 0);
}

#[test]
fn active_skill_rate_modifier_reads_the_authoritative_field_state() {
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
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: FIELD_ORIGIN,
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30865117,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1, -2]);
    execution.context.runtime_target_uid = -1;
    execution.context.logic_target = 201;

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(execution.modifiers.rates.iter().any(|modifier| {
        modifier.opcode == 10001
            && modifier.target_uid == 0
            && modifier.fixed_value() == Some(1_000)
    }));
    assert!(execution.modifiers.rates.iter().any(|modifier| {
        modifier.opcode == 10001
            && modifier.target_uid == -2
            && modifier.fixed_value() == Some(-1_100)
    }));
}

#[test]
fn active_modifier_inheriting_its_condition_target_counts_each_target_separately() {
    crate::test_support::init_config();
    let burn = |uid, layer| BuffInfo {
        uid: Some(uid),
        buff_id: Some(4150001),
        from_uid: Some(10),
        layer: Some(layer),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(10_000),
                    buffs: vec![burn(1, 2)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(10_000),
                    buffs: vec![burn(2, 5)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10001, "SkillRateUp", vec![100]),
        TargetRequest {
            code: 202,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 61203,
        type_name: "PerBuffIdCount".into(),
        kind: ParsedConditionKind::BuffIdCount {
            buff_ids: vec![4150001],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        },
        raw_args: vec!["4150001".into()],
    }];
    slot.condition_target = TargetRequest::self_only();
    slot.target_from_condition = true;
    slot.compiled_route = crate::engine::skill::rule::route::ConditionRoute::compile_for_behavior(
        &slot.conditions,
        &slot.behavior.spec,
    );
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    catalog.insert_damage_rate(100, 500);
    catalog.insert_logic_target(100, 202);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext::default());

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(execution.modifiers.rates.is_empty());
    for (target_uid, expected_count) in [(-1, 2), (-2, 5)] {
        let mut modifiers = execution.modifiers.clone();
        crate::engine::skill::behavior::rate::emit_active_target_attack_attributes(
            &mut modifiers,
            10,
            100,
            100,
            crate::engine::skill::behavior::rate::RateRuntime {
                effects: &catalog,
                managers: &managers,
                pool: &pool,
                context: TargetContext {
                    hit_source_uid: 10,
                    hit_target_uid: target_uid,
                    runtime_target_uid: target_uid,
                    ..Default::default()
                },
            },
            &mut RoundDeterminism::default(),
        );
        assert_eq!(modifiers.rates.len(), expected_count);
        assert!(
            modifiers
                .rates
                .iter()
                .all(|modifier| modifier.target_uid == target_uid)
        );
    }
}

#[test]
fn active_logic_target_modifier_is_deferred_and_collected_per_hit() {
    crate::test_support::init_config();
    let buffs = |ids: &[i32], uid_offset| {
        ids.iter()
            .enumerate()
            .map(|(index, buff_id)| BuffInfo {
                uid: Some(index as i64 + uid_offset),
                buff_id: Some(*buff_id),
                ..Default::default()
            })
            .collect()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    buffs: buffs(&[31450171], 1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(10_000),
                    buffs: buffs(
                        &[31450171, 31440116, 31450111, 31450132, 30250003, 30250001],
                        10,
                    ),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
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
        source_uid: -1,
        skill_id: 103374113,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(10);
    let mut execution = SkillExecution::new(TargetContext::default());

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(execution.modifiers.rates.is_empty());
    let collect_for = |target_uid| {
        let mut modifiers = execution.modifiers.clone();
        crate::engine::skill::behavior::rate::emit_active_target_attack_attributes(
            &mut modifiers,
            -1,
            103374113,
            103374113,
            crate::engine::skill::behavior::rate::RateRuntime {
                effects: &catalog,
                managers: &managers,
                pool: &pool,
                context: TargetContext {
                    active_skill_id: 103374113,
                    active_skill_source_uid: -1,
                    active_skill_is_attack: true,
                    logic_target: 202,
                    hit_source_uid: -1,
                    hit_target_uid: target_uid,
                    runtime_target_uid: target_uid,
                    ..Default::default()
                },
            },
            &mut RoundDeterminism::default(),
        );
        modifiers.rates
    };

    assert!(collect_for(10).is_empty());
    let high_status_rates = collect_for(11);
    assert_eq!(high_status_rates.len(), 1);
    assert_eq!(high_status_rates[0].fixed_value(), Some(-1_000));
}

#[test]
fn buff_feature_event_preserves_the_exact_act_identity() {
    let mut context = TargetContext::default();
    apply_event_context(
        &BattleManagers::seeded(&Fight::default()),
        &SkillEffectCatalog::default(),
        &mut context,
        &BattleEvent::BuffFeatureTriggered(
            crate::engine::event::payload::BuffFeatureTriggeredEvent {
                owner_uid: 10,
                source_uid: 11,
                target_uid: 14,
                buff_uid: 12,
                buff_id: 13,
                act_id: 827,
            },
        ),
    );

    assert_eq!(context.runtime_target_uid, 14);
    assert_eq!(context.triggered_buff_uid, 12);
    assert_eq!(context.triggered_buff_act_id, 827);
}

#[test]
fn reinforced_cast_uses_the_configured_upgrade_effect_without_changing_skill_identity() {
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
                current_hp: Some(700),
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30860143], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30860143,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.extra_skill_kind =
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::Reinforced);

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

    let damage = ops.iter().find_map(|op| match op {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))) => Some(damage),
        RuleOp::Command(BattleCommand::HpBatch(commands)) => commands.iter().find_map(|command| {
            let HpCommand::Damage(damage) = command else {
                return None;
            };
            Some(damage)
        }),
        _ => None,
    });
    assert_eq!(
        damage.map(|damage| (damage.origin.key, damage.amount)),
        Some((DefinitionKey::new(30860143, "SkillDamage"), 3_000))
    );
}
