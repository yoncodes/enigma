use sonettobuf::{BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

use super::*;

#[test]
fn barcarola_resources_require_one_nonzero_configured_delta() {
    assert!(supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![3],
    )));
    assert!(!supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![0],
    )));
    assert!(!supports_recover_power(&ParsedBehavior::new(
        60144,
        "RecoverPower",
        vec![1, 3],
    )));
    assert!(supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![3],
    )));
    assert!(!supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![0],
    )));
    assert!(!supports_team_energy(&ParsedBehavior::new(
        60153,
        "AddTeamEnergy",
        vec![-1],
    )));
}

#[test]
fn exact_red_or_blue_behavior_updates_its_registered_carrier() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1195),
                    buff_id: Some(31100551),
                    from_uid: Some(1),
                    act_common_params: Some(String::new()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60154, "AddRedOrBlueCount", vec![1, 1]);

    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 1,
            source_team: 1,
            target_uid: 1,
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
    )
    .expect("exact behavior registry row must emit its state command");
    let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
        panic!("expected one carrier state command")
    };

    managers.execute_buff(command.clone()).unwrap();

    assert_eq!(
        managers
            .buff
            .snapshot(1, 1195)
            .and_then(|buff| buff.act_common_params),
        Some("897#1".to_owned())
    );
}

#[test]
fn exact_buff_owned_charge_adds_caps_and_projects_absolute_state() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(9),
                    team_type: Some(1),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(1006),
                        buff_id: Some(31460143),
                        from_uid: Some(10),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1139),
                            param: vec![70_000],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut repeated_managers = managers.clone();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    for (delta, expected) in [(70_000, 140_000), (20_000, 150_000)] {
        let behavior = ParsedBehavior::new(60298, "AddMeiLeiErCharge", vec![delta]);
        let definition = super::super::registry::find(&behavior).unwrap();
        assert_eq!(definition.kind, BehaviorKind::AddBuffOwnedCharge);
        assert!(
            definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );

        let ops = rule_ops(
            BehaviorOpContext {
                source_uid: 9,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31460171,
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
        let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
            panic!("expected one capped state command")
        };
        let changes = managers.execute_buff(command.clone()).unwrap();
        let [marker] = changes.act_info_markers.as_slice() else {
            panic!("expected one committed absolute marker")
        };
        assert_eq!(marker.target_uid, 10);
        assert_eq!(marker.buff_uid, 1006);
        assert_eq!(marker.act_id, 1139);
        assert_eq!(marker.params, vec![expected]);
        assert_eq!(marker.str_param.as_deref(), Some(""));
        assert_eq!(marker.team_type, 0);

        assert_eq!(
            managers.buff.snapshot(10, 1006).unwrap().act_info[0].param,
            vec![expected]
        );
    }

    let behavior = ParsedBehavior::new(60298, "AddMeiLeiErCharge", vec![1]);
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 9,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31460171,
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
    assert!(ops.is_empty());

    let repeated_behavior = ParsedBehavior::new(60298, "AddMeiLeiErCharge", vec![40_000]);
    let first = rule_ops(
        BehaviorOpContext {
            source_uid: 9,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31460171,
            transfer_count: 1,
            event: None,
            managers: &repeated_managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &repeated_behavior,
    )
    .unwrap();
    let second = first.clone();
    let markers = [first, second]
        .into_iter()
        .map(|ops| {
            let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
                panic!("expected one capped state command")
            };
            let changes = repeated_managers.execute_buff(command.clone()).unwrap();
            changes.act_info_markers[0].params[0]
        })
        .collect::<Vec<_>>();
    assert_eq!(markers, vec![110_000, 150_000]);
    assert_eq!(
        repeated_managers.buff.snapshot(10, 1006).unwrap().act_info[0].param,
        vec![150_000]
    );
    assert_eq!(
        managers.buff.snapshot(10, 1006).unwrap().act_info[0].param,
        vec![150_000]
    );

    for args in [vec![], vec![0], vec![-1], vec![1, 2]] {
        let behavior = ParsedBehavior::new(60298, "AddMeiLeiErCharge", args);
        let definition = super::super::registry::find(&behavior).unwrap();
        assert!(
            !definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
    }
}

#[test]
fn exact_buff_owned_charge_rejects_missing_or_malformed_carrier_state() {
    crate::test_support::init_config();

    let emit = |buffs| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1),
                    buffs,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31460171,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &ParsedBehavior::new(60298, "AddMeiLeiErCharge", vec![70_000]),
        )
    };
    let carrier = |param| BuffInfo {
        uid: Some(1006),
        buff_id: Some(31460143),
        from_uid: Some(10),
        act_info: vec![sonettobuf::BuffActInfo {
            act_id: Some(1139),
            param,
            str_param: Some(String::new()),
        }],
        ..Default::default()
    };

    assert!(emit(Vec::new()).is_none());
    assert!(emit(vec![carrier(Vec::new())]).is_none());
    assert!(emit(vec![carrier(vec![-1])]).is_none());
    assert!(emit(vec![carrier(vec![150_001])]).is_none());
    assert!(emit(vec![carrier(vec![150_000, 1])]).is_none());

    let mut duplicate = carrier(vec![150_000]);
    duplicate.act_info.push(sonettobuf::BuffActInfo {
        act_id: Some(1139),
        param: vec![150_000],
        str_param: Some(String::new()),
    });
    assert!(emit(vec![duplicate]).is_none());

    let mut string_state = carrier(vec![150_000]);
    string_state.act_info[0].str_param = Some("150000".to_owned());
    assert!(emit(vec![string_state]).is_none());
}

fn consume_buff_charge_behavior(raw_args: [&str; 5]) -> ParsedBehavior {
    ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60305, "ConsumeBuffMeiLeiEr"),
        Vec::new(),
        raw_args.into_iter().map(str::to_owned).collect(),
    )
}

fn rhiannon_resource_fight(attunement_layer: i32, charge: i32) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1006),
                        buff_id: Some(31460143),
                        from_uid: Some(10),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1139),
                            param: vec![charge],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1150),
                        buff_id: Some(31460001),
                        from_uid: Some(10),
                        layer: Some(attunement_layer),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn exact_consume_buff_charge_rewards_emits_captured_active_sequence() {
    crate::test_support::init_config();
    let fight = rhiannon_resource_fight(2, 70_000);
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior =
        consume_buff_charge_behavior(["31460001", "1", "8000", "0", "31460002,2:31460111,1"]);

    let definition = super::super::registry::find(&behavior).unwrap();
    assert_eq!(
        definition.kind,
        BehaviorKind::ConsumeBuffIntoChargeAndRewards
    );
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    assert_eq!(
        <Handler as BehaviorHandler>::references(&behavior).buffs,
        vec![31460001, 31460002, 31460111]
    );

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31460121,
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

    let [
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(consume))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateCappedActState(charge))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(first))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(second))),
    ] = ops.as_slice()
    else {
        panic!("expected consume, committed charge delta, and ordered rewards")
    };
    assert_eq!(consume.target_uid, 10);
    assert_eq!(consume.selector, BuffSelector::ExactId(31460001));
    assert_eq!(consume.amount, 1);
    assert_eq!(consume.depleted, DepletedBuff::Remove);
    assert_eq!(charge.buff_uid, 1006);
    assert_eq!(charge.act_id, 1139);
    assert_eq!(charge.delta, 8_000);
    assert_eq!(charge.maximum, 150_000);
    assert_eq!((first.buff_id, first.amount), (31460002, Some(2)));
    assert_eq!((second.buff_id, second.amount), (31460111, None));
    assert_eq!(
        ops.iter()
            .enumerate()
            .map(|(index, op)| <Handler as BehaviorHandler>::output_owner(&behavior, op, index))
            .collect::<Vec<_>>(),
        vec![
            Some(OutputOwner::CausingEvent),
            None,
            Some(OutputOwner::CausingEvent),
            Some(OutputOwner::CausingEvent),
        ]
    );
    assert_eq!(
        OutputOwner::CausingEvent.resolve(false, false),
        OutputOwner::Skill
    );
    assert_eq!(
        OutputOwner::CausingEvent.resolve(true, false),
        OutputOwner::Parent
    );
}

#[test]
fn exact_consume_buff_charge_rewards_keeps_rewards_at_cap_and_requires_cost() {
    crate::test_support::init_config();
    let behavior = consume_buff_charge_behavior(["31460001", "1", "12500", "1", "31460004,4"]);
    let emit = |fight: &Fight| {
        let managers = BattleManagers::seeded(fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31460171,
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
        .unwrap()
    };

    let at_cap = emit(&rhiannon_resource_fight(1, 150_000));
    let [
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_))),
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(ex_point))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(reward))),
    ] = at_cap.as_slice()
    else {
        panic!("expected consume, Moxie, and reward without a redundant charge marker")
    };
    assert_eq!(ex_point.delta, 1);
    assert_eq!(ex_point.config_effect, 60305);
    assert_eq!(reward.buff_id, 31460004);
    assert_eq!(reward.amount, Some(4));
    assert_eq!(
        at_cap
            .iter()
            .enumerate()
            .map(|(index, op)| <Handler as BehaviorHandler>::output_owner(&behavior, op, index))
            .collect::<Vec<_>>(),
        vec![
            Some(OutputOwner::CausingEvent),
            None,
            Some(OutputOwner::CausingEvent)
        ]
    );

    let mut missing_cost = rhiannon_resource_fight(1, 140_000);
    missing_cost.attacker.as_mut().unwrap().entitys[0]
        .buffs
        .pop();
    assert!(emit(&missing_cost).is_empty());
}

#[test]
fn exact_consume_buff_charge_rewards_support_is_strict() {
    let valid =
        consume_buff_charge_behavior(["31460001", "1", "10000", "1", "31460003,3:31460131,1"]);
    assert!(supports_consume_buff_into_charge_and_rewards(&valid));

    for raw_args in [
        ["31460001", "0", "10000", "1", "31460003,3"],
        ["31460001", "1", "0", "1", "31460003,3"],
        ["31460001", "1", "10000", "2", "31460003,3"],
        ["31460001", "1", "10000", "1", "31460003"],
        ["31460001", "1", "10000", "1", "31460003,3:"],
        ["31460001", "1", "10000", "1", "31460003,3,1"],
    ] {
        assert!(!supports_consume_buff_into_charge_and_rewards(
            &consume_buff_charge_behavior(raw_args)
        ));
    }

    let wrong_field_count = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60305, "ConsumeBuffMeiLeiEr"),
        Vec::new(),
        vec!["31460001".to_owned(), "1".to_owned()],
    );
    assert!(!supports_consume_buff_into_charge_and_rewards(
        &wrong_field_count
    ));
}

#[test]
fn recover_power_and_cast_cards_consumes_only_the_casters_incantations() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                skill_group1: vec![100, 101, 102],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        },
        CardInfo {
            uid: Some(20),
            skill_id: Some(200),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(101),
            ..Default::default()
        },
    ]);
    managers.card.seed(&fight);
    let behavior = ParsedBehavior::new(
        60125,
        "RecoverPowerAndDelCardsUseSkill",
        vec![31050152, 210],
    );
    assert!(supports_recover_power_and_cast_cards(&behavior));
    assert_eq!(
        (super::super::registry::find(&behavior).unwrap().references)(&behavior).skills,
        [31050152]
    );

    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31050131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &crate::engine::skill::target::TargetPool::from_fight(&fight),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: 3,
                ..
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ConsumeForEffect(
                CardConsumeForEffect { indices, .. }
            ))),
            RuleOp::Skill(first),
            RuleOp::Skill(second),
        ] if indices == &[0, 2]
            && first.plan.skill_id == 31050152
            && first.target == SkillTarget::LogicRule(210)
            && first.mode == SkillExecutionMode::Active
            && second.plan.skill_id == 31050152
            && second.target == SkillTarget::LogicRule(210)
            && second.mode == SkillExecutionMode::Active
    ));
}

#[test]
fn add_ex_point_aggregates_fire_count_into_one_command() {
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
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20002, "AddExPoint"),
        vec![1],
        Vec::new(),
    );

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 2,
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange { delta: 2, .. })
        ))]
    ));
    assert_eq!(
        super::super::registry::find(&behavior)
            .unwrap()
            .fire_count_mode,
        super::super::registry::FireCountMode::Transfer
    );
}

#[test]
fn add_ex_point_does_not_write_a_special_resource() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ex_point_type: Some(ExPointKind::Faith.as_wire()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20002, "AddExPoint"),
        vec![5],
        Vec::new(),
    );

    let ops = rule_ops(
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
    )
    .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn committed_card_ranks_scale_the_configured_power_gain() {
    crate::test_support::init_config();
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
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243111),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243112),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31243113),
            ..Default::default()
        },
    ]);
    while !managers.card.hand().is_empty() {
        managers.card.play_card(0, None, None, None).unwrap();
    }
    managers
        .execute_card(crate::engine::manager::card::CardCommand::QueueUseCard(
            crate::engine::manager::card::CardQueueUse {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(60070, "AddUseSkillCard"),
                },
                card_index: 4,
                card: sonettobuf::CardInfo {
                    uid: Some(10),
                    skill_id: Some(370001002),
                    ..Default::default()
                },
                team_type: 1,
                source_skill_id: 370001010,
                action: None,
            },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60115, "TotalSkillRankToPower", vec![3000, 4]);

    let ops = rule_ops(
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Eureka(
            EurekaCommand::Change(EurekaChange {
                power_id: 4,
                delta: 27,
                ..
            })
        ))]
    ));
}

#[test]
fn del_ex_point_validates_and_emits_the_configured_loss() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ex_point: Some(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30001, "DelExPoint"),
        vec![1],
        vec!["1".into()],
    );

    let definition = super::super::registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 11,
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
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                target_uid: 11,
                delta: -1,
                ..
            })
        ))]
    ));
}

#[test]
fn team_energy_uses_the_shared_team_gauge() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60153, "AddTeamEnergy", vec![3]);

    let ops = rule_ops(
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::Enable { max: None },
                ..
            })),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::ChangeValue { delta: 3 },
                ..
            }))
        ]
    ));
}

fn per_type_buff_energy_ops(fight: &Fight) -> (Vec<RuleOp>, i32, i32) {
    let mut managers = BattleManagers::seeded(fight);
    let key = crate::engine::mechanic::impromptu::team_energy_key(1);
    managers
        .execute_gauge(GaugeCommand::new(
            crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                key: crate::engine::skill::rule::DefinitionKey::new(0, "Test"),
            },
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let before = managers.buff.buff_id_amount(10, 303901411);
    let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60264,
            "PerTypeBuffAddEnergyToTeam",
        ),
        Vec::new(),
        vec!["303901411".to_owned(), "1".to_owned(), "1".to_owned()],
    );
    let ops = super::super::rule_ops(
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
    )
    .expect("exact per-type buff team-energy behavior must emit");
    let after = managers.buff.buff_id_amount(10, 303901411);
    (ops, before, after)
}

#[test]
fn per_type_buff_team_energy_counts_exact_layers_for_the_source_team() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2001),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(4),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2002),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    let (ops, before, after) = per_type_buff_energy_ops(&fight);
    let [RuleOp::Command(BattleCommand::Gauge(command))] = ops.as_slice() else {
        panic!("expected one aggregate team-energy command")
    };

    assert_eq!(before, 7);
    assert_eq!(after, 7);
    assert_eq!(
        command.key,
        crate::engine::mechanic::impromptu::team_energy_key(1)
    );
    assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 7 });
    assert_eq!(command.source_uid, 10);
    assert_eq!(command.config_effect, 60264);
}

#[test]
fn per_type_buff_team_energy_is_a_noop_without_the_exact_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2004),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    let (ops, before, after) = per_type_buff_energy_ops(&fight);
    assert!(ops.is_empty());
    assert_eq!(before, 0);
    assert_eq!(after, 0);
}

fn per_type_buff_emitter_energy_ops(
    fight: &Fight,
    target_uid: i64,
    enable_gauge: bool,
) -> (Vec<RuleOp>, i32, i32) {
    let mut managers = BattleManagers::seeded(fight);
    let key =
        crate::engine::mechanic::impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    if enable_gauge {
        managers
            .execute_gauge(GaugeCommand::new(
                crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                    key: crate::engine::skill::rule::DefinitionKey::new(0, "Test"),
                },
                key,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
    }
    let before = managers.buff.buff_id_amount(10, 303901411);
    let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60266,
            "PerTypeBuffAddEnergyToEmitter",
        ),
        Vec::new(),
        vec!["303901411".to_owned(), "2".to_owned()],
    );
    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid,
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
    )
    .expect("exact per-type emitter-energy behavior must emit");
    let after = managers.buff.buff_id_amount(10, 303901411);
    (ops, before, after)
}

#[test]
fn per_type_buff_emitter_energy_counts_source_layers_for_any_target_without_consuming_them() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2001),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(4),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2002),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };

    for target_uid in [10, crate::engine::manager::emitter::UID] {
        let (ops, before, after) = per_type_buff_emitter_energy_ops(&fight, target_uid, true);
        let [RuleOp::Command(BattleCommand::Gauge(command))] = ops.as_slice() else {
            panic!("expected one aggregate emitter-energy command")
        };

        assert_eq!(before, 7);
        assert_eq!(after, 7);
        assert_eq!(
            command.key,
            crate::engine::mechanic::impromptu::inspiration_key(
                crate::engine::manager::emitter::UID,
            )
        );
        assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 14 });
        assert_eq!(command.source_uid, 10);
        assert_eq!(command.config_effect, 60266);
    }
}

#[test]
fn per_type_buff_emitter_energy_is_a_noop_without_layers_or_gauge() {
    crate::test_support::init_config();
    let no_layers = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(2003),
                        buff_id: Some(303901412),
                        from_uid: Some(10),
                        layer: Some(9),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2004),
                        buff_id: Some(303901411),
                        from_uid: Some(10),
                        layer: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: None,
        ..Default::default()
    };
    let (ops, before, after) = per_type_buff_emitter_energy_ops(&no_layers, 10, true);
    assert!(ops.is_empty());
    assert_eq!((before, after), (0, 0));

    let with_layers = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2001),
                    buff_id: Some(303901411),
                    from_uid: Some(10),
                    layer: Some(7),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (ops, before, after) = per_type_buff_emitter_energy_ops(&with_layers, 10, false);
    assert!(ops.is_empty());
    assert_eq!((before, after), (7, 7));
}

#[test]
fn emitter_energy_uses_the_enabled_inspiration_gauge() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let key =
        crate::engine::mechanic::impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    managers
        .execute_gauge(GaugeCommand::new(
            crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                key: crate::engine::skill::rule::DefinitionKey::new(0, "Test"),
            },
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60152, "AddEmitterEnergy", vec![6]);

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: crate::engine::manager::emitter::UID,
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            key: command_key,
            operation: GaugeOperation::ChangeValue { delta: 6 },
            source_uid: 10,
            ..
        }))] if *command_key == key
    ));
}

#[test]
fn exact_conduit_counter_behavior_commits_typed_round_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    for (kind, delta, expected) in [
        (2, 2, ConduitCounterKind::Activation),
        (1, 4, ConduitCounterKind::EnergyAccumulation),
        (1, 6, ConduitCounterKind::EnergyAccumulation),
    ] {
        let behavior = ParsedBehavior::new(60297, "AddDeviceCounter", vec![kind, delta]);
        let definition = super::super::registry::find(&behavior).unwrap();
        assert_eq!(definition.kind, BehaviorKind::AddConduitCounter);
        assert!(
            definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
        let ops = rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31447002,
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
        let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
            panic!("expected one Conduit counter command");
        };
        let change = managers.conduit.execute(*command).unwrap();
        assert!(matches!(
            change,
            crate::engine::manager::conduit::ConduitChange::CounterChanged {
                kind: actual,
                requested_delta,
                ..
            } if actual == expected && requested_delta == delta
        ));
    }

    for args in [
        vec![],
        vec![1],
        vec![0, 2],
        vec![3, 2],
        vec![1, 0],
        vec![2, -1],
        vec![1, 2, 3],
    ] {
        let behavior = ParsedBehavior::new(60297, "AddDeviceCounter", args);
        let definition = super::super::registry::find(&behavior).unwrap();
        assert!(
            !definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
    }
}

#[test]
fn exact_conduit_power_behavior_commits_typed_energy() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60291, "AddDevicePower", vec![1, 4, 1]);

    let definition = super::super::registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490111,
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
    let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
        panic!("expected one Conduit command");
    };
    let change = managers.conduit.execute(*command).unwrap();

    assert_eq!(managers.conduit.power(1, 1), 4);
    assert!(matches!(
        change,
        crate::engine::manager::conduit::ConduitChange::PowerChanged {
            power_id: 1,
            applied_delta: 4,
            kind: ConduitPowerChangeKind::Interval,
            ..
        }
    ));
}

#[test]
fn exact_conduit_ex_point_behavior_uses_the_entity_resource_owner() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                ex_point_type: Some(4),
                ex_point_max: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60292, "AddDeviceExPoint", vec![8]);

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490111,
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                target_uid: 10,
                delta: 8,
                effect_type,
                config_effect,
                origin,
                ..
            })
        ))] if *effect_type == EffectType::Expointchange as i32
            && *config_effect == 0
            && origin.key == crate::engine::skill::rule::DefinitionKey::new(
                60292,
                "AddDeviceExPoint",
            )
    ));
}

#[test]
fn exact_conduit_group_behavior_changes_the_manager_owned_selection() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60293, "SetDeviceSkillIndex", vec![3]);
    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31490161,
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
    let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
        panic!("expected one Conduit command");
    };

    let change = managers.conduit.execute(*command).unwrap();
    assert!(matches!(
        change,
        crate::engine::manager::conduit::ConduitChange::SkillGroupChanged {
            origin,
            source_uid: 10,
            team: 1,
            group: 3,
            ..
        } if origin.key == crate::engine::skill::rule::DefinitionKey::new(
            60293,
            "SetDeviceSkillIndex",
        )
    ));
    assert_eq!(managers.conduit.selected_group(10), Some(3));
}

#[test]
fn interval_conduit_skill_stops_the_exact_active_skill() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    for opcode in [100034, 60294] {
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .conduit
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(opcode, "StopDeviceSkill"),
                },
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 3,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::new(opcode, "StopDeviceSkill", Vec::new());
        let ops = rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31490111,
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
        let [RuleOp::Command(BattleCommand::Conduit(command))] = ops.as_slice() else {
            panic!("expected one Conduit command");
        };

        assert!(matches!(
            managers.conduit.execute(*command).unwrap(),
            crate::engine::manager::conduit::ConduitChange::SkillStopped {
                origin,
                source_uid: 10,
                team: 1,
                skill_id: 31490111,
            } if origin.key == crate::engine::skill::rule::DefinitionKey::new(
                opcode,
                "StopDeviceSkill",
            )
        ));
        assert!(!managers.conduit.can_begin_skill(10, 31490111, 0));
        assert!(managers.conduit.can_begin_skill(10, 31490121, 0));
        assert!(matches!(
            managers.conduit.execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490111,
                cost_reduction: 0,
            }),
            Err(crate::engine::manager::conduit::ConduitError::StoppedSkill(
                31490111
            ))
        ));

        managers
            .conduit
            .execute(ConduitCommand::RestartDevice { source_uid: 10 })
            .unwrap();
        assert!(managers.conduit.can_begin_skill(10, 31490111, 0));
    }
}

#[test]
fn absorb_ex_point_emits_loss_then_actual_gain() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ex_point: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ex_point: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30011, "AbsorbExPoint"),
        vec![5],
        Vec::new(),
    );

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &crate::engine::skill::target::TargetPool::from_fight(&fight),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                ExPointChange {
                    target_uid: 11,
                    delta: -2,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                ExPointChange {
                    target_uid: 10,
                    delta: 2,
                    ..
                }
            )))
        ]
    ));
}

#[test]
fn crit_power_progress_counts_a_critical_incantation_once_per_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(0),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60187, "AddPowerByCritCount", vec![2, 1]);

    for expected in [0, 1] {
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext {
            action_crit_count: 3,
            ..Default::default()
        };
        let op = rule_ops(
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
        )
        .unwrap()
        .pop()
        .unwrap();
        let RuleOp::Command(BattleCommand::Eureka(command)) = op else {
            panic!("expected progress-gated Eureka command")
        };

        managers.execute_eureka(command).unwrap();
        assert_eq!(
            managers.eureka.get(10, EUREKA_RESOURCE_ID).current,
            expected
        );
    }

    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        critical_action_count: 6,
        ..Default::default()
    };
    let op = rule_ops(
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
    )
    .unwrap()
    .pop()
    .unwrap();
    let RuleOp::Command(BattleCommand::Eureka(command)) = op else {
        panic!("expected progress-gated Eureka command")
    };
    managers.execute_eureka(command).unwrap();
    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 4);
}

#[test]
fn average_life_redistributes_team_hp_by_max_hp_ratio() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(300),
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
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20011, "AverageLife"),
        vec![0],
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = rule_ops(
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
    )
    .unwrap();

    let values = ops
        .into_iter()
        .map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::SetCurrent(set))) => {
                (set.target_uid, set.value)
            }
            _ => panic!("expected current-HP set command"),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec![(10, 50), (11, 150)]);
}
