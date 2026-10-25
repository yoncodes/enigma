use super::*;

#[test]
fn magic_circle_attr_scope_one_is_owner_and_scope_two_is_team() {
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 433311,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(60076, "MagicCircleAttr", vec![1, 301, 40, 2, 301, 40]),
            TargetRequest::self_only(),
        )],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    passive_skill: vec![433311],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 1,
                    duration: 1,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let context = TargetContext::default();
    let mut owner_modifiers = crate::engine::skill::action::SkillModifiers::default();
    emit_passive_attack_attributes(
        &mut owner_modifiers,
        10,
        100,
        &[433311],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context,
        },
        &mut RoundDeterminism::default(),
    );
    let mut ally_modifiers = crate::engine::skill::action::SkillModifiers::default();
    emit_passive_attack_attributes(
        &mut ally_modifiers,
        11,
        100,
        &[],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context,
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(owner_modifiers.attack_attributes.len(), 2);
    assert_eq!(
        ally_modifiers.attack_attributes,
        vec![(AttrId::PoisonDmgBonus, 40)]
    );
}

#[test]
fn excess_crit_conversion_is_returned_to_the_current_cast() {
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(60228, "CritRateAlter2", vec![500]),
            TargetRequest::self_only(),
        )],
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
    let pool = TargetPool::from_fight(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        10,
        100,
        &[200],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext::default(),
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(modifiers.excess_crit_conversion_rate, 500);
}

#[test]
fn field_self_skills_use_the_registered_attack_modifier_pipeline() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 100051,
                    duration: 1,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    emit_passive_attack_attributes(
        &mut modifiers,
        11,
        1,
        &[],
        RateRuntime {
            effects: crate::engine::skill::effect::catalog::global(),
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                blood_pool_max: 84,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(modifiers.attack_attributes, vec![(AttrId::ExtraDmg, 300)]);
}

#[test]

fn excess_crit_conversion_is_a_supported_static_destination() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut target = TargetContext::default();

    for (opcode, type_name, rate) in [
        (40001, "CritRateAlter", 1000),
        (100023, "CritRateAlter2", 1000),
        (60228, "CritRateAlter2", 500),
    ] {
        let behavior = ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(opcode, type_name),
            vec![rate],
            Vec::new(),
        );
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

        assert!(crate::engine::skill::behavior::has_destination(&behavior));
        assert!(matches!(
            crate::engine::skill::behavior::rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 10,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(ops) if ops.is_empty()
        ));
        assert_eq!(modifiers.excess_crit_conversion_rate, rate);
    }
}

#[test]
fn bullet_crit_conversion_counts_configured_buff_group_types() {
    crate::test_support::init_config();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(60086, "BulletCritRateAlter", vec![650, 100, 5]),
        TargetRequest::self_only(),
    );
    assert!(crate::engine::skill::behavior::has_destination(
        &slot.behavior
    ));
    slot.conditions = vec![
        ParsedCondition {
            opcode: 77203,
            type_name: "HasBuffGroup".into(),
            kind: ParsedConditionKind::BuffGroup(vec![5]),
            raw_args: vec!["5".into()],
        },
        ParsedCondition {
            opcode: 501203,
            type_name: "UseHurtSkill".into(),
            kind: ParsedConditionKind::UseHurtSkill,
            raw_args: Vec::new(),
        },
    ];
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 31020162,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(31020112),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020113),
                        ..Default::default()
                    },
                ],
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
        100,
        &[31020162],
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext {
                active_skill_is_attack: true,
                ..Default::default()
            },
        },
        &mut RoundDeterminism::default(),
    );

    assert_eq!(modifiers.excess_crit_conversion_rate, 850);
}
