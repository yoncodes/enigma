use super::*;

#[test]
fn origin_damage_accepts_only_a_known_basis_attribute_and_nonnegative_rate() {
    assert!(supports_origin_damage(&ParsedBehavior::from_spec(
        BehaviorSpec::new(30014, "OriginDamage"),
        vec![0, AttrId::Hp.id(), 200],
        Vec::new(),
    )));
    assert!(!supports_origin_damage(&ParsedBehavior::from_spec(
        BehaviorSpec::new(30014, "OriginDamage"),
        vec![2, AttrId::Hp.id(), 200],
        Vec::new(),
    )));
    assert!(supports_origin_damage(&ParsedBehavior::from_spec(
        BehaviorSpec::new(30015, "OriginDamageCanCrit"),
        vec![0, AttrId::Attack.id(), 400],
        Vec::new(),
    )));
}

#[test]
fn origin_damage_routes_exact_max_hp_basis_through_the_behavior_registry() {
    init_config();
    let mut source = entity(10, 1, 1, 100, 100);
    source.current_hp = Some(2_000);
    source.attr.as_mut().unwrap().hp = Some(2_000);
    let mut target_entity = entity(-1, 2, 1, 1_000, 100);
    target_entity.buffs = vec![BuffInfo {
        uid: Some(1),
        buff_id: Some(2112021),
        from_uid: Some(-1),
        duration: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![target_entity],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(30014, "OriginDamage"),
        vec![0, AttrId::Hp.id(), 200],
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
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

    let [RuleOp::Command(BattleCommand::Hp(command))] = ops.as_slice() else {
        panic!("expected one HP command, got {ops:#?}");
    };
    assert!(matches!(
        command,
        HpCommand::Damage(HpDamage {
            amount: 400,
            effect_kind: DamageEffectKind::Genesis,
            ..
        })
    ));
    let changes = managers.execute_hp(*command).unwrap();
    assert_eq!(changes.damage.unwrap().amount, 400);
    assert_eq!(managers.hp.current(-1), 600);
}

#[test]
fn origin_damage_by_buff_group_counts_instances_and_genesis_modifiers() {
    crate::test_support::init_config();
    let mut source = entity(10, 1, 1, 1000, 100);
    source.buffs = vec![BuffInfo {
        buff_id: Some(30980152),
        uid: Some(1),
        ..Default::default()
    }];
    let mut target = entity(-1, 2, 1, 1000, 100);
    target.buffs = vec![
        BuffInfo {
            buff_id: Some(30980126),
            uid: Some(2),
            ..Default::default()
        },
        BuffInfo {
            buff_id: Some(30980111),
            uid: Some(3),
            ..Default::default()
        },
        BuffInfo {
            buff_id: Some(30980111),
            uid: Some(4),
            ..Default::default()
        },
        BuffInfo {
            buff_id: Some(30980111),
            uid: Some(5),
            ..Default::default()
        },
    ];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![target],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60127, "OriginDamageByAttrAndBuffGroupSize"),
        vec![1, AttrId::Attack as i32, 700, 7],
        Vec::new(),
    );

    assert_eq!(managers.buff.buff_group_amount(-1, 7), 3);
    assert_eq!(
        origin::buff_group_amount(
            10,
            -1,
            origin::OriginRuntime {
                managers: &managers,
                pool: &pool,
                extra_action: false,
            },
            &[],
            false,
            &behavior,
        ),
        Some(3465)
    );
}
#[test]
fn team_attribute_origin_damage_sums_each_main_allys_current_missing_hp() {
    init_config();
    let mut source = entity(10, 1, 1, 1_000, 100);
    source.current_hp = Some(800);
    let mut ally = entity(20, 1, 1, 1_000, 100);
    ally.current_hp = Some(700);
    let target = entity(-1, 2, 1, 1_000, 100);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source, ally],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![target],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60146, "OriginDamageByTeamAttr"),
        vec![1, AttrId::LostHp as i32, 150],
        Vec::new(),
    );

    assert_eq!(
        origin::team_attribute_amount(
            10,
            -1,
            origin::OriginRuntime {
                managers: &managers,
                pool: &pool,
                extra_action: false,
            },
            &behavior,
        ),
        Some(75)
    );
}

#[test]
fn extra_action_origin_damage_uses_its_exact_critical_formula() {
    init_config();
    let mut source = entity(10, 1, 1, 1_000, 100);
    source.buffs = vec![BuffInfo {
        buff_id: Some(31050186),
        uid: Some(1),
        from_uid: Some(10),
        ..Default::default()
    }];
    let target_entity = entity(-1, 2, 1, 1_000, 100);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![target_entity],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(400),
            cri_dmg: Some(1750),
            ..Default::default()
        },
    );
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(30015, "OriginDamageCanCrit"),
        vec![0, AttrId::Attack.id(), 1000],
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        active_skill_id: 100,
        extra_skill_kind: crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction.id(),
        ..Default::default()
    };

    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 100,
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
    assert!(
        matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                HpLoss {
                    amount: 2275,
                    hurt: Some(HurtInfoData { is_crit: true, .. }),
                    ..
                }
            )))]
        ),
        "{ops:#?}"
    );
}

#[test]
fn direct_damage_uses_manager_combat_stats_instead_of_stale_pool_values() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1_000, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.register(&entity(10, 1, 1, 2_000, 100));
    managers.attribute.register(&entity(-1, 2, 1, 0, 500));
    let command = resolve_attack_command(
        &AttackPlan {
            source_uid: 10,
            target_uid: -1,
            skill_id: 1,
            rate: 1_000,
            rate_terms: Vec::new(),
            attack_attributes: Vec::new(),
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            assassinate: false,
            main_target: true,
            extra_skill_kind: 0,
            additional_enabled: false,
            additional_is_crit: None,
        },
        DamageRuntime {
            fight_version: 6,
            pool: &pool,
            attributes: &managers.attribute,
            buffs: &managers.buff,
            target_buffs: &managers.buff,
            hp: &managers.hp,
            fields: None,
            emitter: None,
            team_inspiration: 0,
        },
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "Damage"),
        },
    )
    .unwrap();

    assert!(matches!(
        command,
        HpCommand::Damage(HpDamage { amount: 1_500, .. })
    ));
}
