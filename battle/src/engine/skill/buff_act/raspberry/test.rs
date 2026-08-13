use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn parses_raspberry_payload_offsets() {
    let act = RaspberryBuffAct::from_feature(&feature(vec![
        1042, 100, 100, 700, 150, 203, 50, 211, 33, 1, 40,
    ]))
    .unwrap();

    assert_eq!(act.loss_from_current_hp(10_000), 1_000);
    assert_eq!(act.shared_gain_from_loss(1_000), 700);
    assert_eq!(act.max_cap_from_source_hp(20_000), 3_000);
    assert!(act.crossed_cap(2_000, 3_000, 3_000));
    assert!(!act.crossed_cap(3_000, 3_000, 3_000));
}

#[test]
fn ignores_non_raspberry_or_dead_features() {
    assert_eq!(
        RaspberryBuffAct::from_feature(&feature(vec![1021, 8, 1])),
        None
    );

    let mut dead = feature(vec![1042, 100, 100, 700, 150]);
    dead.owner_alive = false;
    assert_eq!(RaspberryBuffAct::from_feature(&dead), None);
}

#[test]
fn projects_fractional_threshold_attributes_without_ten_point_floor() {
    assert_eq!(stepped_attribute(1_167, 50), 58);
    assert_eq!(stepped_attribute(1_200, 50), 60);
    assert_eq!(stepped_attribute(1_909, 33), 62);
}

#[test]
fn shadow_feast_uses_the_configured_reduced_attribute_rates() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31250151),
                        from_uid: Some(20),
                        act_common_params: Some("1000#3000".to_owned()),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(31),
                        buff_id: Some(31250121),
                        from_uid: Some(20),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1041),
                            param: vec![1000],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(attribute_delta(&managers.buff, 10, AttrId::CriticalDmg), 90);
    assert_eq!(
        attribute_delta(&managers.buff, 10, AttrId::UltimateMight),
        59
    );
}

#[test]
fn add_count_uses_the_registered_raspberry_capacity_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(77),
                    buff_id: Some(31250151),
                    from_uid: Some(11),
                    act_common_params: Some("1000#1200".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60231, "RaspberryAddCount"),
    };
    let Some(ops) = add_count_rule_ops(&managers, origin, 11, 11, AttrId::CurrentHp, 40) else {
        panic!("expected a Raspberry capacity command");
    };
    let [RuleOp::Command(BattleCommand::RaspberryAddCount(command))] = ops.as_slice() else {
        panic!("expected one Raspberry add-count intent");
    };

    let CapacityResult::Applied(changes) =
        execute_add_count(&mut managers, *command).unwrap().unwrap()
    else {
        panic!("expected an applied Raspberry capacity change");
    };

    assert!(changes.ex_point.is_some());
    assert_eq!(managers.hp.max(11), 20_200);
    assert_eq!(
        managers
            .buff
            .snapshot(11, 77)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("1200#1200")
    );
}

#[test]
fn add_count_at_cap_reports_a_sync_without_mutating_hp() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(77),
                    buff_id: Some(31250151),
                    from_uid: Some(11),
                    act_common_params: Some("1200#1200".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let result = execute_add_count(
        &mut managers,
        AddCountCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60231, "RaspberryAddCount"),
            },
            source_uid: 11,
            target_uid: 11,
            attr_id: AttrId::CurrentHp,
            rate: 40,
        },
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        result,
        CapacityResult::AtCap(CapacityAtCap {
            target_uid: 11,
            buff_uid: 77,
            buff_act_id: 1042,
            current: 1200,
            cap: 1200,
            max_hp: 20_000,
        })
    );
    assert_eq!(managers.hp.current(11), 10_000);
    assert_eq!(managers.hp.max(11), 20_000);
}

#[test]
fn queued_add_count_reads_the_source_after_the_previous_commit() {
    crate::test_support::init_config();
    let entity = |uid, buff_uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(20_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31250151),
            from_uid: Some(11),
            act_common_params: Some("0#5000".to_owned()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(11, 77), entity(12, 78)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let command = |target_uid| AddCountCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60231, "RaspberryAddCount"),
        },
        source_uid: 11,
        target_uid,
        attr_id: AttrId::CurrentHp,
        rate: 40,
    };

    execute_add_count(&mut managers, command(11)).unwrap();
    execute_add_count(&mut managers, command(12)).unwrap();

    assert_eq!(managers.hp.current(11), 10_400);
    assert_eq!(
        managers
            .buff
            .snapshot(12, 78)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("416#5000")
    );
}

#[test]
fn big_skill_transfers_other_allies_capacity_into_data_selected_feast() {
    crate::test_support::init_config();
    let entity = |uid, current, max| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(max),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(uid + 100),
            buff_id: Some(31250151),
            from_uid: Some(10),
            act_common_params: Some(format!("{current}#3000")),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 700, 20_700),
                entity(11, 1_000, 21_000),
                entity(12, 500, 20_500),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60233, "RaspberryBigSkill"),
    };

    let ops = big_skill_rule_ops(&managers, origin, 10, 10, 350, 31250221).unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::RaspberryCapacity(CapacityCommand { target_uid: 11, current: 0, delta: -1_000, .. })),
            RuleOp::Command(BattleCommand::RaspberryCapacity(CapacityCommand { target_uid: 12, current: 0, delta: -500, .. })),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantStateful(BuffGrantChild { buff_id: 31250221, act_info: Some(act_info), .. }))),
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(MaxHpAdjust { target_uid: 10, delta: 525, .. }))),
        ] if act_info.as_slice() == [BuffActInfo {
            act_id: Some(1041),
            param: vec![525],
            str_param: Some(String::new()),
        }]
    ));
}

#[test]
fn primary_team_raspberry_removes_temporary_capacity_with_the_feast() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31250151),
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
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31250151,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: crate::engine::event::subscription::SubscriptionKey::new(
            crate::engine::event::kind::EventKind::BuffRemoved,
            DefinitionKey::new(1042, "Raspberry"),
        ),
        act_type: "Raspberry".to_owned(),
        effect_time: 103,
        effect_condition: 0,
        args: vec![100, 100, 700, 150, 203, 50, 211, 33, 1, 40],
        raw: "1042#100#100#700#150#203#50#211#33#1#40".to_owned(),
    };

    let ops = rule_ops(
        &managers,
        &subscriber,
        &BattleEvent::BuffRemoved(crate::engine::event::payload::BuffChangeEvent {
            source_uid: 10,
            target_uid: 10,
            buff_uid: 30,
            buff_id: 31250221,
            before_amount: 1,
            after_amount: 0,
            act_id: 1041,
            act_value: 525,
        }),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
            MaxHpAdjust {
                target_uid: 10,
                delta: -525,
                ..
            }
        )))]
    ));
}

fn feature(values: Vec<i32>) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 20,
        buff_uid: 30,
        buff_id: 40,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "Raspberry".to_owned(),
        effect_time: 103,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    }
}
