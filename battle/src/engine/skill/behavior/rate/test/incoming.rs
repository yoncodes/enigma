use super::*;

#[test]
fn target_missing_hp_scales_only_the_incoming_damage_reduction_lane() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_roots(config::configs::get(), [342440140], []);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
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
                current_hp: Some(500),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![342440140],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        incoming_target_attack_modifiers(
            10,
            -1,
            200,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes,
        vec![(AttrId::DmgBonus, -100); 5]
    );
}

#[test]
fn incoming_condition_target_queries_the_attacker() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                passive_skill: vec![2205],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        incoming_target_attack_modifiers(
            10,
            -1,
            200,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes,
        vec![(AttrId::DmgBonus, -150)]
    );
}

#[test]
fn incoming_modifiers_include_passives_linked_by_target_buffs() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(23300010),
                    from_uid: Some(-1),
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

    assert_eq!(managers.buff.passive_skill_links_for(-1).len(), 1);
    assert_eq!(
        incoming_target_attack_modifiers(
            10,
            -1,
            200,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes,
        vec![(AttrId::PlaymodeDmgIncrease, -300)]
    );
}

#[test]
fn linked_burn_attacker_modifier_uses_the_configured_psychube_value() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(436125),
                    from_uid: Some(-1),
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

    assert_eq!(
        incoming_target_attack_modifiers(
            10,
            -1,
            200,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes,
        vec![(AttrId::DmgBonus, -40)]
    );
}

#[test]
fn linked_target_passive_selects_the_stronger_afflatus_modifier() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(4),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(23300010),
                    from_uid: Some(-1),
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

    assert_eq!(
        incoming_target_attack_modifiers(
            10,
            -1,
            200,
            RateRuntime {
                effects: &effects,
                managers: &managers,
                pool: &pool,
                context: TargetContext::default(),
            },
            &mut RoundDeterminism::default(),
        )
        .attack_attributes,
        vec![(AttrId::PlaymodeDmgIncrease, 300)]
    );
}

#[test]
fn forced_afflatus_advantage_disables_not_restrained_modifier() {
    crate::test_support::init_config();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(10004, "AttrFix", vec![205, -300]),
        TargetRequest::self_only(),
    );
    slot.conditions.push(ParsedCondition {
        opcode: 47204,
        type_name: String::new(),
        kind: ParsedConditionKind::HurtNotRestrained,
        raw_args: Vec::new(),
    });
    let mut effects = SkillEffectCatalog::default();
    effects.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(2),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(30860101),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(4),
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
            10,
            -1,
            200,
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
fn incoming_career_ratio_fix_is_preserved_as_a_typed_modifier() {
    crate::test_support::init_config();
    let effects = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(6),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(5),
                current_hp: Some(100),
                passive_skill: vec![23390182],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    let modifiers = incoming_target_attack_modifiers(
        10,
        -1,
        31130114,
        RateRuntime {
            effects: &effects,
            managers: &managers,
            pool: &pool,
            context: TargetContext::default(),
        },
        &mut RoundDeterminism::default(),
    );

    assert!(modifiers.attack_attributes.is_empty());
    assert_eq!(modifiers.career_ratio_bonus, 300);
}
