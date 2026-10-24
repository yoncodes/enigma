use super::*;

#[test]
fn target_count_rate_is_global_and_scales_with_living_enemies() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60234,
            "AddSkillRateByTargetCount",
        ),
        vec![4400],
        Vec::new(),
    );
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 1,
                source_team: 1,
                target_uid: 1,
                active_skill_id: 30073335,
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
    assert_eq!(modifiers.rates.len(), 1);
    assert_eq!(modifiers.rates[0].amount.fixed_value(), Some(13_200));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}

#[test]
fn conduit_rate_survives_a_later_group_switch() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60291, "AddDevicePower"),
    };
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
                    delta: 3,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490121,
                cost_reduction: 0,
            },
        )
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::CommitSkillCost {
                source_uid: 10,
                skill_id: 31490121,
            },
        )
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(100030, "TwinsUpByCounter", vec![0, 0, 900, 6, 0, 0, 900]);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31490121,
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

    assert_eq!(modifiers.rates[0].fixed_value(), Some(2700));
    assert!(matches!(
        ops.as_slice(),
        [crate::engine::skill::rule::output::RuleOp::EffectMarker {
            effect_num: 3,
            config_effect: 100030,
            ..
        }]
    ));
}

#[test]
fn conduit_unique_skill_uses_all_energy_and_its_documented_thresholds() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(1),
                ex_point: Some(100),
                ex_point_type: Some(4),
                ex_point_max: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60291, "AddDevicePower"),
    };
    for (power_id, delta) in [(0, 1), (1, 6), (2, 6)] {
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::ChangePower(
                    crate::engine::manager::conduit::ConduitPowerChange {
                        origin,
                        source_uid: 10,
                        team: 1,
                        power_id,
                        delta,
                        kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                    },
                ),
            )
            .unwrap();
    }
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(100031, "TwinsPowerUp"),
        Vec::new(),
        [
            "0,1,2", "2600", "6", "9", "12", "13000", "13000", "13000", "201", "1000", "213",
            "700", "1000",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31490151,
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

    assert_eq!(modifiers.rates[0].fixed_value(), Some(72_800));
    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::CriticalRate, 1000), (AttrId::Penetration, 700)]
    );
    assert_eq!(modifiers.excess_crit_conversion_rate, 1000);
    assert_eq!(modifiers.attack_career, Some(1));
    assert_eq!(modifiers.additional_attack_career, Some(2));
    assert!(matches!(
        ops.as_slice(),
        [
            crate::engine::skill::rule::output::RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Conduit(
                    crate::engine::manager::conduit::ConduitCommand::ClearPowers {
                        power_ids: _,
                        ..
                    }
                ),
            ),
            crate::engine::skill::rule::output::RuleOp::EffectMarker {
                effect_num: 13,
                config_effect: 100031,
                ..
            },
        ]
    ));
    let crate::engine::skill::rule::output::RuleOp::Command(
        crate::engine::skill::rule::output::BattleCommand::Conduit(
            crate::engine::manager::conduit::ConduitCommand::ClearPowers { power_ids, .. },
        ),
    ) = &ops[0]
    else {
        unreachable!()
    };
    assert_eq!(power_ids, &[0, 1, 2]);
}
