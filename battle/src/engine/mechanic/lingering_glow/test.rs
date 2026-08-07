use super::*;
use crate::engine::{
    manager::BattleManagers,
    skill::{
        behavior::classify::BehaviorSpec,
        effect::slot::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        target::TargetRequest,
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn round_start_bonus_snapshots_remaining_pool_then_depletes_half() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31340003),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31340008),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(22),
                        buff_id: Some(30810301),
                        from_uid: Some(11),
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
    let origin = buff_act::feature_command_origin(
        &managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| buff_act::is_kind(feature, BuffActKind::HeatScaleTag))
            .unwrap(),
    )
    .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key(1),
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key(1),
            GaugeOperation::ChangeValue { delta: 45 },
        ))
        .unwrap();

    let ops = round_start_attribute_rule_ops_for_team(
        &managers,
        crate::engine::skill::effect::catalog::global(),
        1,
    );

    assert_eq!(ops.len(), 5);
    assert!(ops[..2].iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
            BuffGrantChild { act_info: Some(info), .. }
        ))) if info[0].param == [22]
    )));
    assert!(matches!(
        ops[2],
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            operation: GaugeOperation::ChangeValue { delta: -23 },
            raw_delta: Some(-22_500),
            ..
        }))
    ));
    assert!(matches!(
        ops[3],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                target_uid: 11,
                buff_uid: 22,
                act_info: Some(ref info),
                ..
            }
        ))) if info[0].act_id == Some(1062) && info[0].param == [1_125]
    ));
    assert!(matches!(
        ops[4],
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: 11,
            act_id: 1062,
            ref params,
            ..
        }) if params == &[1_125]
    ));
    let RuleOp::Command(BattleCommand::Buff(counter)) = ops[3].clone() else {
        panic!("counter state command")
    };
    managers.execute_buff(counter).unwrap();
    let next = round_start_attribute_rule_ops_for_team(
        &managers,
        crate::engine::skill::effect::catalog::global(),
        1,
    );
    assert!(matches!(
        next[4],
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult { ref params, .. })
            if params == &[2_250]
    ));
    assert_eq!(managers.gauge.get(key(1)).unwrap().current, 45);
}

#[test]
fn direct_lingering_glow_gain_uses_the_active_team_modifier() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31270410),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60191, "BloodPoolValueChange"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key(1),
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();

    let ops = value_change_rule_ops(
        &managers,
        GaugeCommand::new(origin, key(1), GaugeOperation::ChangeValue { delta: 20 })
            .with_raw_delta(20_000)
            .with_progress_raw_delta(20_000),
    );

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            operation: GaugeOperation::AccumulateRawValue {
                amount: 21_600,
                stream: 60191,
            },
            raw_delta: Some(21_600),
            progress_raw_delta: Some(21_600),
            ..
        }))]
    ));
}

#[test]
fn activation_counter_floors_raw_progress_and_crystals_keep_separate_owners() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31340163,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60246, "HeatScaleUseSkillAddCount"),
                vec![75_000],
                vec!["75000".to_owned()],
            ),
            TargetRequest::self_only(),
        )],
    });
    let card = feature(
        10,
        1,
        20,
        31340008,
        "CardNotCalSize",
        vec![951, 31340161, 31340163],
    );
    let tag = feature(10, 1, 21, 31340003, "HeatScaleTag", vec![1052]);
    let halo = feature(-1, 2, 30, 31340001, "MasterHalo", vec![771, 31340001]);
    let use_skill = feature(10, 1, 22, 31340004, "HeatScaleUseSkill", vec![1050]);
    let decrement = feature(
        10,
        1,
        23,
        31340005,
        "HeatScaleDecrCounter",
        vec![1062, 2_000],
    );
    let features = vec![tag, card, halo, use_skill, decrement];
    let mut managers = BattleManagers::default();
    let mut runtime = LingeringGlowRuntime::default();
    let enable = enable_rule_ops(&managers.gauge, &features, &catalog)
        .pop()
        .unwrap();
    let RuleOp::Command(BattleCommand::Gauge(command)) = enable.output else {
        panic!("gauge command");
    };
    managers.execute_gauge(command).unwrap();
    assert!(runtime.register(&managers.gauge, enable.create));

    let input = burn_or_halo_rule_op(
        &managers.gauge,
        &features,
        heat_scale::BurnOrHaloAdded {
            source_team: 1,
            target_uid: -1,
            buff_uid: 30,
            added_layers: 1,
            alive_enemy_index: 2,
            alive_enemy_count: 3,
        },
    )
    .unwrap();
    let RuleOp::Command(BattleCommand::Gauge(command)) = input.output else {
        panic!("gauge command");
    };
    let change = managers.execute_gauge(command).unwrap();
    assert!(runtime.apply_change(change, input.raw_delta));

    assert_eq!(managers.gauge.get(key(1)).unwrap().current, 1);
    assert_eq!(runtime.raw_value(1), 1_666);
    assert_eq!(
        visible_counter_info(&managers.gauge, &features, 1)
            .unwrap()
            .current,
        1
    );
    assert_eq!(
        runtime.decrement_counter_info(&features, 1).unwrap().value,
        3_332
    );
    assert!(managers.emanation.select(10, 110));
    assert_eq!(managers.emanation.counts(10), [1, 1, 0]);
    assert_eq!(managers.emanation.choose(10, 0), Some(0));
}

#[test]
fn ready_cast_is_derived_from_the_subscriber_and_current_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(40),
                    buff_id: Some(999),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin =
        buff_act::feature_command_origin(&feature(10, 1, 20, 31340003, "HeatScaleTag", vec![1052]))
            .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key(1),
            GaugeOperation::Enable { max: Some(200) },
        ))
        .unwrap();
    managers
        .execute_gauge(
            GaugeCommand::new(origin, key(1), GaugeOperation::ChangeValue { delta: 200 })
                .with_progress_raw_delta(150_000),
        )
        .unwrap();
    let mut runtime = LingeringGlowRuntime::default();
    assert!(runtime.register(
        &managers.gauge,
        HeatScaleCreate {
            team: 1,
            amount: 200,
            raw_amount: 200_000,
            source_buff_id: 31340003,
        },
    ));
    assert!(managers.emanation.select(10, 2));
    let use_skill = crate::engine::skill::subscriber::BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 30,
        buff_id: 31340004,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        key: crate::engine::event::subscription::SubscriptionKey::new(
            crate::engine::event::kind::EventKind::AllyAction,
            crate::engine::skill::rule::DefinitionKey::new(1050, "HeatScaleUseSkill"),
        ),
        act_type: "HeatScaleUseSkill".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "1050#150000#111,222,333#0#0#999#-50000".to_owned(),
        args: vec![150_000],
    };

    let cast = ready_cast_rule_ops(
        &managers.gauge,
        &managers.buff,
        &managers.emanation,
        &SkillEffectCatalog::default(),
        &use_skill,
    )
    .unwrap();

    assert_eq!(cast.cast.skill_id, 333);
    assert_eq!(cast.cast.trigger_value, 150);
    assert_eq!(cast.cast.current, 50);
    assert_eq!(cast.cast.consume_buff_id, Some(999));
    assert!(matches!(
        cast.outputs.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::IdOrType(999),
                amount: 1,
                ..
            }))),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::ConsumeAccumulated {
                    listener_uid: 30,
                    listener_opcode: 1050,
                    amount: 100,
                },
                ..
            })),
            RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                target_uid: 10,
                buff_uid: 30,
                act_id: 1050,
                params,
                ..
            }),
            RuleOp::Skill(invocation),
        ] if params == &[50]
            && invocation.plan.skill_id == 333
            && invocation.mode == SkillExecutionMode::Active
    ));

    let progress_consume = cast.outputs.iter().find_map(|op| match op {
        RuleOp::Command(BattleCommand::Gauge(command))
            if matches!(command.operation, GaugeOperation::ConsumeAccumulated { .. }) =>
        {
            Some(*command)
        }
        _ => None,
    });
    managers.execute_gauge(progress_consume.unwrap()).unwrap();
    assert_eq!(managers.gauge.get(key(1)).unwrap().current, 200);
    assert_eq!(managers.gauge.accumulated_value(key(1), 30, 1050), Some(50));
}

#[test]
fn bloodtithe_and_lingering_glow_cannot_both_enable_for_one_team() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31340163,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60246, "HeatScaleUseSkillAddCount"),
                vec![75_000],
                vec!["75000".to_owned()],
            ),
            TargetRequest::self_only(),
        )],
    });
    let glow_features = vec![
        feature(10, 1, 20, 31340003, "HeatScaleTag", vec![1052]),
        feature(
            10,
            1,
            21,
            31340008,
            "CardNotCalSize",
            vec![951, 31340161, 31340163],
        ),
    ];
    let blood_features = vec![
        feature(10, 1, 30, 6270501, "BloodPoolTag", vec![953]),
        feature(11, 1, 31, 6270501, "BloodPoolTag", vec![953]),
    ];

    let mut glow_first = BattleManagers::default();
    let enable = enable_rule_ops(&glow_first.gauge, &glow_features, &catalog)
        .pop()
        .unwrap();
    let RuleOp::Command(BattleCommand::Gauge(command)) = enable.output else {
        panic!("gauge command");
    };
    glow_first.execute_gauge(command).unwrap();
    assert!(
        bloodtithe::rule::enable_rule_op(&glow_first, &blood_features[0], &blood_features)
            .is_none()
    );

    let mut blood_first = BattleManagers::default();
    let RuleOp::Command(BattleCommand::Gauge(command)) =
        bloodtithe::rule::enable_rule_op(&blood_first, &blood_features[0], &blood_features)
            .unwrap()
    else {
        panic!("gauge command");
    };
    blood_first.execute_gauge(command).unwrap();
    assert!(enable_rule_ops(&blood_first.gauge, &glow_features, &catalog).is_empty());
}

fn feature(
    owner_uid: i64,
    team_type: i32,
    buff_uid: i64,
    buff_id: i32,
    act_type: &str,
    values: Vec<i32>,
) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: 10,
        buff_uid,
        buff_id,
        amount: 1,
        team_type,
        owner_alive: true,
        act_type: act_type.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    }
}
