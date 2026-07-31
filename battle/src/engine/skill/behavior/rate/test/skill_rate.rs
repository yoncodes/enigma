use super::*;

#[test]
fn status_skill_rate_counts_configured_statuses_up_to_its_limit() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(400401),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(301),
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
    let capped = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10002, "SkillRateUp1"),
        vec![1_000, 1],
        vec!["1000".into(), "1".into(), "1,5".into()],
    );
    let uncapped = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10002, "SkillRateUp1"),
        vec![200, 99],
        vec!["200".into(), "99".into(), "1,5".into()],
    );

    assert_eq!(status_skill_rate(&managers.buff, 10, &capped), 1_000);
    assert_eq!(status_skill_rate(&managers.buff, 10, &uncapped), 400);

    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        30230134,
        0,
        &capped,
    ));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}

#[test]
fn target_status_skill_rate_is_scoped_to_the_attacked_entity() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(301),
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
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10003, "SkillRateUp2"),
        vec![400, 1],
        vec!["400".into(), "1".into(), "2,4,6".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        runtime_target_uid: -1,
        ..Default::default()
    };

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 1163855041,
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
    assert_eq!(modifiers.rates[0].fixed_value(), Some(400));
    assert_eq!(modifiers.rates[0].target_uid, -1);
}

#[test]
fn self_buff_type_rate_scales_by_the_configured_buff_amount() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31490008),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::new(60182, "SkillRateUpBySelfBuffType", vec![31490008, 450]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    assert!(supports_self_buff_type_rate(&behavior));
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        31490141,
        0,
        &behavior,
    ));
    assert_eq!(modifiers.rates[0].fixed_value(), Some(1_350));
    assert!(!supports_self_buff_type_rate(&ParsedBehavior::new(
        60182,
        "SkillRateUpBySelfBuffType",
        vec![31490008],
    )));
}

#[test]
fn card_rank_skill_rate_reads_the_selected_action_area_and_caps_the_total() {
    crate::test_support::init_config();
    let mut cards = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950111),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950112),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950113),
            ..Default::default()
        },
    ]);
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();
    cards.queue_use_card(crate::engine::manager::card::QueuedUseCard {
        card_index: 4,
        card: sonettobuf::CardInfo {
            uid: Some(-1),
            skill_id: Some(370001002),
            ..Default::default()
        },
        team_type: 1,
        source_skill_id: 370001010,
        action: None,
    });
    let behavior = |cap| {
        ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60067,
                "SkillRateUpCardLevel",
            ),
            vec![cap],
            vec!["2,3".into(), "300,500".into(), cap.to_string()],
        )
    };

    assert_eq!(card_rank_skill_rate(&cards, &behavior(2_500)), 1_300);
    assert_eq!(card_rank_skill_rate(&cards, &behavior(700)), 700);

    let mut managers = BattleManagers::default();
    managers.card = cards;
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        30950111,
        0,
        &behavior(2_500),
    ));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}

#[test]
fn heat_scale_rate_uses_the_configured_scale_and_limit() {
    assert_eq!(heat_scale_skill_rate(12, &[1000, 20, 9999]), 240);
    assert_eq!(heat_scale_skill_rate(20, &[10000, 350, 9999]), 700);
}

#[test]
fn purple_crystal_resolves_raw_lingering_glow_when_damage_is_planned() {
    use crate::engine::{
        manager::gauge::{GaugeCommand, GaugeManager, GaugeOperation},
        mechanic::lingering_glow,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    let key = lingering_glow::key(1);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60243, "CrystalAddSkillRate"),
    };
    let mut gauges = GaugeManager::default();
    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: Some(1000) },
        ))
        .unwrap();
    let modifier = SkillRateModifier::new(
        0,
        60243,
        crate::engine::skill::action::SkillRateAmount::gauge_raw(key, 1_000_000, 4, 1, 1000),
        true,
    );

    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 120_000,
                stream: 60243,
            },
        ))
        .unwrap();
    assert_eq!(modifier.amount.resolve(&gauges), 480);

    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 30_000,
                stream: 60243,
            },
        ))
        .unwrap();
    assert_eq!(modifier.amount.resolve(&gauges), 600);
}

#[test]
fn selected_crystal_rank_row_carries_its_fixed_mass_and_focus_damage() {
    assert_eq!(
        crystal_fixed_skill_rates(&[1200, 800, 1]),
        Some((1200, 800))
    );
}

#[test]
fn crystal_damage_components_declare_their_career_scope() {
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddSkillRate,
        false
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddSkillRate,
        true
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddCardRank,
        false
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddCardRank,
        true
    ));
}

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
fn consume_ex_point_uses_only_additional_moxie() {
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    emit(
        &mut modifiers,
        10,
        10,
        None,
        31140131,
        3,
        &ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60174,
                "ConsumeExPointAddAttr",
            ),
            vec![211, 30, 1, 5, 221, 1000],
            Vec::new(),
        ),
    );

    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::UltimateMight, 90)]
    );
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
    for power_id in [1, 2] {
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::ChangePower(
                    crate::engine::manager::conduit::ConduitPowerChange {
                        origin,
                        source_uid: 10,
                        team: 1,
                        power_id,
                        delta: 6,
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

    assert_eq!(modifiers.rates[0].fixed_value(), Some(70_200));
    assert_eq!(
        modifiers.attack_attributes,
        vec![(AttrId::CriticalRate, 1000), (AttrId::Penetration, 700)]
    );
    assert_eq!(modifiers.excess_crit_conversion_rate, 1000);
    assert!(matches!(
        ops.as_slice(),
        [
            crate::engine::skill::rule::output::RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::ExPoint(
                    crate::engine::manager::ex_point::ExPointCommand::Set(
                        crate::engine::manager::ex_point::ExPointSet { value: 0, .. }
                    )
                )
            ),
            crate::engine::skill::rule::output::RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Conduit(
                    crate::engine::manager::conduit::ConduitCommand::ClearPowers {
                        power_ids: [1, 2],
                        ..
                    }
                )
            ),
            crate::engine::skill::rule::output::RuleOp::EffectMarker {
                effect_num: 12,
                config_effect: 100031,
                ..
            }
        ]
    ));
}
