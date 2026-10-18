use super::*;

#[test]
fn linked_shell_attributes_keep_extra_action_kinds_distinct() {
    use crate::engine::skill::condition::extra::ExtraSkillKind;

    assert_eq!(
        plan::LinkedDamagePlan::shell_attributes(0, false),
        (true, false)
    );
    assert_eq!(
        plan::LinkedDamagePlan::shell_attributes(ExtraSkillKind::ExtraAction.id(), false),
        (true, true)
    );
    assert_eq!(
        plan::LinkedDamagePlan::shell_attributes(ExtraSkillKind::FollowUp.id(), false),
        (true, false)
    );
    assert_eq!(
        plan::LinkedDamagePlan::shell_attributes(ExtraSkillKind::Riposte.id(), false),
        (false, false)
    );
    assert_eq!(
        plan::LinkedDamagePlan::shell_attributes(ExtraSkillKind::Riposte.id(), true),
        (true, false)
    );
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

    for (is_crit, expected_damage) in [(true, 990), (false, 600)] {
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
        let damage = emission.ops.iter().find_map(|output| match &output.op {
            RuleOp::Command(BattleCommand::HpBatch(commands)) => {
                commands.iter().find_map(|command| match command {
                    crate::engine::manager::hp::HpCommand::Damage(damage) => Some(damage.amount),
                    _ => None,
                })
            }
            _ => None,
        });
        assert_eq!(damage, Some(expected_damage));
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
        plan::LinkedDamagePlan::activations(&invocation, &managers, &execution)
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
        for activation in plan::LinkedDamagePlan::activations(&invocation, &managers, &execution) {
            execution.activate_additional_damage(activation.additional);
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
