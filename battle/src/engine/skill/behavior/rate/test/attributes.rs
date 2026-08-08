use super::*;

#[test]
fn attr_fix_emits_a_typed_attack_attribute() {
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    assert!(emit(
        &mut modifiers,
        10,
        10,
        None,
        30630122,
        0,
        &ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(10004, "AttrFix"),
            vec![AttrId::Penetration as i32, 250],
            Vec::new(),
        ),
    ));
    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::Penetration, 250)]
    );
}

#[test]
fn outgoing_restraint_modifier_only_applies_to_the_weaker_afflatus() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(1),
                current_hp: Some(100),
                passive_skill: vec![72008],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    career: Some(4),
                    current_hp: Some(100),
                    passive_skill: vec![72008],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    career: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    career: Some(1),
                    weak_careers: vec![1],
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let collect = |target_uid| {
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        emit_passive_attack_attributes(
            &mut modifiers,
            10,
            30630121,
            &[72008],
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext {
                    hit_source_uid: 10,
                    hit_target_uid: target_uid,
                    ..Default::default()
                },
            },
            &mut RoundDeterminism::default(),
        );
        modifiers.attack_attributes
    };

    assert_eq!(collect(-1), vec![(AttrId::Penetration, 300)]);
    assert!(collect(-2).is_empty());
    assert_eq!(collect(-3), vec![(AttrId::Penetration, 300)]);
    assert!(
        incoming_target_attack_modifiers(
            10,
            -1,
            30630121,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes
        .is_empty()
    );
}

#[test]
fn passive_attr_fix_uses_held_moxie_for_the_current_attack() {
    let mut effects = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10004, "AttrFix", vec![211, 24]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 58201,
        type_name: String::new(),
        kind: ParsedConditionKind::PerExPoint { threshold: 1 },
        raw_args: vec!["1".into()],
    }];
    effects.insert(ParsedSkillEffect {
        skill_id: 434415,
        slots: vec![slot],
    });
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(3),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        31140131,
        &[434415],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &TargetPool::from_fight(&Fight {
                attacker: Some(FightTeam {
                    entitys: vec![FightEntityInfo {
                        uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            context: TargetContext::default(),
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(
        modifiers
            .attack_attributes
            .iter()
            .filter(|(attr, _)| *attr == AttrId::UltimateMight)
            .map(|(_, delta)| delta)
            .sum::<i32>(),
        72
    );
}

#[test]
fn owner_wide_passive_skill_rate_repeats_are_global_to_the_attack() {
    let mut effects = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10001, "SkillRateUp", vec![300]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![
        ParsedCondition {
            opcode: 507201,
            type_name: "UseSkillId".into(),
            kind: ParsedConditionKind::ActiveSkillId(vec![200]),
            raw_args: vec!["200".into()],
        },
        ParsedCondition {
            opcode: 578,
            type_name: "TeamInjuryCountRound".into(),
            kind: ParsedConditionKind::TeamInjuryCountRound { max_count: 20 },
            raw_args: vec!["20".into()],
        },
    ];
    effects.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        200,
        &[100],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &TargetPool::from_fight(&fight),
            context: TargetContext {
                active_skill_id: 200,
                team_injury_count_round: 3,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(modifiers.rates.len(), 3);
    assert!(modifiers.rates.iter().all(|rate| rate.target_uid == 0));
    assert_eq!(
        modifiers
            .rates
            .iter()
            .map(|rate| rate.amount.fixed_value().unwrap_or_default())
            .sum::<i32>(),
        900
    );
}

#[test]
fn source_modifier_with_target_999_uses_the_current_hit_target() {
    let mut effects = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10004, "AttrFix", vec![205, 60]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 19201,
        type_name: "HasBuffId".into(),
        kind: ParsedConditionKind::BuffId {
            mode: crate::engine::skill::condition::buff::BuffConditionMode::Present,
            buff_ids: vec![4150001],
        },
        raw_args: vec!["4150001".into()],
    }];
    slot.target_from_condition = true;
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    effects.insert(ParsedSkillEffect {
        skill_id: 432315,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                passive_skill: vec![432315],
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(4150001),
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
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        200,
        &[432315],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                hit_source_uid: 10,
                hit_target_uid: -1,
                runtime_target_uid: -1,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(modifiers.attack_attributes, vec![(AttrId::DmgBonus, 60)]);
}

#[test]
fn liang_yue_poison_scaling_uses_the_compiled_target_group_count() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let poison = |uid, amount| BuffInfo {
        uid: Some(uid),
        buff_id: Some(30560101),
        from_uid: Some(10),
        count: Some(amount),
        layer: Some(amount),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    buffs: vec![poison(1, 2)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    buffs: vec![poison(2, 1)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        31100563,
        &[31100563],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                active_skill_id: 31100563,
                active_skill_source_uid: 10,
                active_skill_is_attack: true,
                logic_target: 202,
                hit_source_uid: 10,
                hit_target_uid: -1,
                runtime_target_uid: -1,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::DmgBonus, 100); 3]
    );
}

#[test]
fn bullet_triggered_buff_is_not_applied_before_its_event() {
    crate::test_support::init_config();
    let mut effects = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10004, "AttrFix", vec![205, 100]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 649210,
        type_name: "TriggerBullet".into(),
        kind: ParsedConditionKind::BuffFeatureTriggered { act_id: 827 },
        raw_args: Vec::new(),
    }];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    effects.insert(ParsedSkillEffect {
        skill_id: 433711,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![433711],
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31020111),
                    from_uid: Some(10),
                    count: Some(5),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        31020141,
        &[433711],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &TargetPool::from_fight(&fight),
            context: TargetContext::default(),
        },
        &mut RoundDeterminism::default(),
    );

    assert!(modifiers.attack_attributes.is_empty());
}

#[test]
fn passive_attr_fix_can_gate_a_mass_extra_action() {
    crate::test_support::init_config();
    let mut effects = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10004, "AttrFix", vec![214, 280]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![
        ParsedCondition {
            opcode: 53201,
            type_name: String::new(),
            kind: ParsedConditionKind::DamageTargetCountKind(2),
            raw_args: vec!["2".into()],
        },
        ParsedCondition {
            opcode: 403201,
            type_name: String::new(),
            kind: ParsedConditionKind::ExtraAction {
                mode:
                    crate::engine::skill::condition::extra::ExtraActionConditionMode::ActiveAction,
                kinds: vec![1],
            },
            raw_args: vec!["1".into()],
        },
    ];
    effects.insert(ParsedSkillEffect {
        skill_id: 432315,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        30630122,
        &[432315],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &TargetPool::from_fight(&fight),

            context: TargetContext {
                extra_skill_kind:
                    crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction.id(),
                damage_target_count_kind: 2,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::IncantationMight, 280)]
    );
}

#[test]
fn virtual_emitter_does_not_trigger_character_target_attack_modifiers() {
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(10004, "AttrFix", vec![205, -300]),
            TargetRequest::self_only(),
        )],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert!(
        incoming_target_attack_modifiers(
            crate::engine::manager::emitter::UID,
            -1,
            2240001,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes
        .is_empty()
    );
    assert!(
        incoming_target_attack_modifiers(
            10,
            -1,
            2240001,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes
        .is_empty(),
        "ordinary source modifiers must not become incoming target modifiers"
    );
}
