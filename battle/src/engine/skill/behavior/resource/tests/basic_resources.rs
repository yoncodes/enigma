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
        vec![-2],
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
fn full_recovery_uses_the_owned_eureka_maximum() {
    let entity = FightEntityInfo {
        uid: Some(10),
        power_infos: vec![PowerInfo {
            power_id: Some(EUREKA_RESOURCE_ID),
            num: Some(3),
            max: Some(10),
        }],
        ..Default::default()
    };
    let mut managers = BattleManagers::default();
    managers.eureka.register(&entity);
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60144, "RecoverPower", vec![-1]);

    let ops = crate::engine::skill::behavior::rule_ops(
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
                power_id: EUREKA_RESOURCE_ID,
                delta: 7,
                ..
            })
        ))]
    ));
}

#[test]
fn synchronization_gain_uses_the_resource_wire_identity() {
    let managers = BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let mut behavior = ParsedBehavior::new(100014, "EzioAddSynchronization", vec![5]);
    behavior.config_effect = 100014;

    let ops = rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 312301611,
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
    .expect("the exact synchronization behavior emits its resource change");

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(change)))]
            if change.delta == 5 && change.config_effect == 0
    ));
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

    let ops = super::super::super::rule_ops(
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
        let definition = super::super::super::registry::find(&behavior).unwrap();
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
        let definition = super::super::super::registry::find(&behavior).unwrap();
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

    let definition = super::super::super::registry::find(&behavior).unwrap();
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
        OutputOwner::CausingEvent.resolve(None, false),
        OutputOwner::Skill
    );
    assert_eq!(
        OutputOwner::CausingEvent.resolve(
            Some(crate::engine::event::kind::EventKind::SkillAction),
            false,
        ),
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
