use super::*;

#[test]
fn per_hp_passive_repeats_exact_attack_attributes_for_the_current_target() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = effects.get(30070541).expect("passive must be parsed");
    assert_eq!(effect.slots.len(), 2);
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                passive_skill: vec![30070541],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
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
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        30073335,
        &[30070541],
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

    assert_eq!(
        modifiers.attack_attributes,
        [
            vec![(AttrId::Penetration, 100); 5],
            vec![(AttrId::CriticalRate, 80); 5],
        ]
        .concat()
    );
}

#[test]
fn visible_status_threshold_still_collects_attack_skill_rate() {
    crate::test_support::init_config();
    let condition = ParsedCondition {
        opcode: 42203,
        type_name: "HasTypeBuffMoreThan".to_owned(),
        kind: crate::engine::skill::condition::registry::parse(
            42203,
            "HasTypeBuffMoreThan",
            &["5".into(), "1,5".into()],
        )
        .unwrap(),
        raw_args: vec!["5".into(), "1,5".into()],
    };
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10001, "SkillRateUp", vec![500]),
        TargetRequest {
            code: crate::engine::skill::target::request::SOURCE_TARGET_CODE,
            raw: Vec::new(),
        },
    );
    slot.condition_target = slot.target.clone();
    slot.conditions = vec![condition];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let mut buffs = [31450171, 31450143, 31440116, 31450111]
        .into_iter()
        .enumerate()
        .map(|(index, buff_id)| BuffInfo {
            uid: Some(index as i64 + 1),
            buff_id: Some(buff_id),
            from_uid: Some(10),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    buffs.push(BuffInfo {
        uid: Some(5),
        buff_id: Some(103374114),
        from_uid: Some(-2),
        layer: Some(2),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                passive_skill: vec![100],
                buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let context = TargetContext {
        active_skill_id: 200,
        active_skill_source_uid: 10,
        active_skill_is_attack: true,
        hit_source_uid: 10,
        hit_target_uid: -1,
        runtime_target_uid: -1,
        ..Default::default()
    };
    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        200,
        &[100],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context,
        },
        &mut RoundDeterminism::default(),
    );

    assert!(
        modifiers
            .rates
            .iter()
            .any(|rate| rate.opcode == 10001 && rate.fixed_value() == Some(500)),
    );
}

#[test]
fn bullet_types_repeat_each_configured_attack_attribute() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(722501),
                        uid: Some(1),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(722502),
                        uid: Some(2),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020111),
                        uid: Some(3),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020112),
                        uid: Some(4),
                        count: Some(1),
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
                current_hp: Some(1_000),
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
        310201361,
        &[310201621],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                active_skill_source_uid: 10,
                hit_source_uid: 10,
                hit_target_uid: -1,
                runtime_target_uid: -1,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    for attr in [
        AttrId::DmgBonus,
        AttrId::IncantationMight,
        AttrId::UltimateMight,
    ] {
        assert_eq!(
            modifiers
                .attack_attributes
                .iter()
                .filter(|(actual, _)| *actual == attr)
                .map(|(_, delta)| delta)
                .sum::<i32>(),
            150
        );
    }
}

#[test]
fn consume_ex_point_rate_uses_only_additional_moxie() {
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    emit(
        &mut modifiers,
        10,
        10,
        None,
        31140131,
        1,
        &ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60174,
                "ConsumeExPointAddAttr",
            ),
            vec![211, 30, 3, 5, 221, 1000],
            Vec::new(),
        ),
    );

    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::UltimateMight, 30)]
    );
}

#[test]
fn consume_ex_point_spends_the_excess_and_rewards_the_lowest_ally() {
    crate::test_support::init_config();
    let entity = |uid, ex_point| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        ex_point: Some(ex_point),
        ex_point_max: Some(10),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 9), entity(20, 0), entity(30, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60174, "ConsumeExPointAddAttr"),
        vec![211, 60, 1, 5, 221, 1000],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        active_skill_id: 311401351,
        active_skill_source_uid: 10,
        ..Default::default()
    };

    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 311401351,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert_eq!(target.additional_moxie, 4);
    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::UltimateMight, 240)]
    );
    assert!(matches!(
        &ops[0],
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Spend(change)))
            if change.target_uid == 10 && change.delta == -4
    ));
    assert_eq!(ops.len(), 5);
    assert!(ops[1..].iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::LowestExPoint(change))
            if change.source_uid == 10
                && change.candidates == [(20, 0), (30, 0)]
                && change.delta == 1
    )));

    let stacked = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 311401351,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    assert_eq!(stacked.len(), 4);
    assert!(stacked.iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::LowestExPoint(change))
            if change.source_uid == 10
                && change.candidates == [(20, 0), (30, 0)]
                && change.delta == 1
    )));
    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::UltimateMight, 240), (AttrId::UltimateMight, 240)]
    );

    let event = crate::engine::event::payload::BattleEvent::RoundStart;
    assert!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 311401351,
                transfer_count: 1,
                event: Some(&event),
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .is_none()
    );
}

#[test]
fn consume_ex_point_passive_stays_unsupported_until_distribution_is_owned() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let active = effects.get(311401351).expect("active skill must be parsed");
    let active_slot = active
        .slots
        .iter()
        .find(|slot| slot.behavior.spec.kind == BehaviorKind::ConsumeExPointAddAttr)
        .expect("active consume behavior must be parsed");
    assert!(crate::engine::skill::behavior::is_slot_supported(
        active_slot
    ));
    let mut mass_target = active_slot.clone();
    mass_target.target.code = 202;
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &mass_target
    ));
    let mut mass_condition_target = active_slot.clone();
    mass_condition_target.condition_target.code = 202;
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &mass_condition_target
    ));
    let mut extra_condition = active_slot.clone();
    extra_condition.compiled_route.as_mut().unwrap().branches[0]
        .conditions
        .push(crate::engine::skill::rule::RuleDescriptor::new(
            crate::engine::skill::rule::RuleDomain::Condition,
            crate::engine::skill::rule::DefinitionKey::new(210, "None"),
        ));
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &extra_condition
    ));
    let mut multiple_branches = active_slot.clone();
    let branch = multiple_branches.compiled_route.as_ref().unwrap().branches[0].clone();
    multiple_branches
        .compiled_route
        .as_mut()
        .unwrap()
        .branches
        .push(branch);
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &multiple_branches
    ));
    let mut raw_condition_args = active_slot.clone();
    raw_condition_args.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "203#1");
    raw_condition_args.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile_for_behavior(
            &raw_condition_args.conditions,
            &raw_condition_args.behavior.spec,
        );
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &raw_condition_args
    ));
    let mut negated_condition = active_slot.clone();
    negated_condition.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "203!");
    negated_condition.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile_for_behavior(
            &negated_condition.conditions,
            &negated_condition.behavior.spec,
        );
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        &negated_condition
    ));
    let passive = effects.get(82600002).expect("passive must be parsed");
    let slot = passive
        .slots
        .iter()
        .find(|slot| slot.behavior.spec.kind == BehaviorKind::ConsumeExPointAddAttr)
        .expect("consume behavior must be parsed");
    assert!(!crate::engine::skill::behavior::is_slot_supported(slot));
    assert!(
        effects
            .compiled_subscription_lanes(82600002)
            .unwrap()
            .iter()
            .all(
                |(slot_index, _)| passive.slots[*slot_index].behavior.spec.kind
                    != BehaviorKind::ConsumeExPointAddAttr
            )
    );

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ex_point: Some(9),
                passive_skill: vec![82600002],
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
        311401351,
        &[82600002],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                active_skill_is_attack: true,
                active_skill_source_uid: 10,
                active_skill_id: 311401351,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert!(modifiers.attack_attributes.is_empty());
}

#[test]
fn same_named_rate_behavior_with_an_unowned_opcode_is_rejected() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec {
            key: crate::engine::skill::behavior::classify::BehaviorKey::new(99999, "SkillRateUp"),
            kind: BehaviorKind::SkillRateUp,
        },
        vec![4500],
        Vec::new(),
    );
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    assert!(!emit(&mut modifiers, 1, -1, None, 100, 0, &behavior,));
    assert_eq!(modifiers, Default::default());
}
