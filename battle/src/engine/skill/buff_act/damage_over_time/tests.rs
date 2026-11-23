use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::{bus::EventBus, kind::EventKind, subscription::SubscriptionKey},
    runtime::executor::execute_rule_op,
};

#[test]
fn burn_formula_uses_owner_attack_rate_per_stack() {
    assert_eq!(1441 * 40 * 8 / 1000, 461);
    assert_eq!(1441 * 40 * 3 / 1000, 172);
}

#[test]
fn radiance_halves_its_stacks() {
    assert_eq!(settled_layer(DamageOverTimeKind::Radiance, &[], 4), Some(2));
    assert_eq!(settled_layer(DamageOverTimeKind::Radiance, &[], 1), Some(0));
    assert_eq!(settled_layer(DamageOverTimeKind::Burn, &[], 4), Some(2));
    assert_eq!(settled_layer(DamageOverTimeKind::Burn, &[], 1), Some(0));
    assert_eq!(
        settled_layer(DamageOverTimeKind::Dot, &[0, AttrId::Hp.id(), 50], 18),
        Some(9)
    );
    assert_eq!(
        settled_layer(DamageOverTimeKind::Dot, &[0, AttrId::Attack.id(), 50], 18),
        None
    );
}

#[test]
fn periodic_genesis_damage_emits_its_registered_marker_with_the_transaction() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
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
    let mut subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31340001,
        team_type: 2,
        owner_alive: true,
        amount: 2,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(1048, "Radiance"),
        ),
        act_type: "Radiance".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![0, 102, 40],
        raw: "1048#0#102#40".to_owned(),
    };

    assert_eq!(registry::runtime_marker(subscriber.key.definition), None);
    assert!(matches!(
        damage_rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber,
        )
        .as_slice(),
        [
            RuleOp::BuffFeatureMarker {
                target_uid: -1,
                effect_num: 31340001,
                buff_act_id: 1048,
                ..
            },
            RuleOp::Command(BattleCommand::Hp(_))
        ]
    ));

    subscriber.key = SubscriptionKey::new(
        EventKind::RoundEnd,
        crate::engine::skill::rule::DefinitionKey::new(726, "Burn"),
    );
    subscriber.act_type = "Burn".to_owned();
    assert_eq!(registry::runtime_marker(subscriber.key.definition), None);

    subscriber.key = SubscriptionKey::new(
        EventKind::SkillAction,
        crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
    );
    subscriber.act_type = "Dot".to_owned();
    subscriber.effect_time = 210;
    subscriber.args = vec![1, AttrId::Hp.id(), 200];
    assert!(matches!(
        damage_rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber,
        )
        .as_slice(),
        [
            RuleOp::BuffFeatureMarker {
                effect_type,
                buff_act_id: 203,
                ..
            },
            RuleOp::Command(BattleCommand::Hp(_))
        ] if *effect_type == EffectType::Dot as i32
    ));
}

#[test]
fn poison_tick_emits_its_registered_marker_before_the_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let buff_uid = managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: -1,
                buff_id: 11410012,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 10,
        buff_uid,
        buff_id: 11410012,
        team_type: 2,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(803, "Poison"),
        ),
        act_type: "Poison".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![300, 0],
        raw: "803#300#0".to_owned(),
    };

    let ops = damage_rule_ops(
        &managers,
        &pool,
        &mut RoundDeterminism::default(),
        &subscriber,
    );
    assert!(
        matches!(
            ops.as_slice(),
            [
                RuleOp::BuffFeatureMarker {
                    effect_type,
                    effect_num: 11410012,
                    buff_act_id: 803,
                    ..
                },
                RuleOp::Command(BattleCommand::Hp(_))
            ] if *effect_type == EffectType::Poison as i32
        ),
        "{ops:?}"
    );
}

#[test]
fn layered_dot_emits_one_current_hp_settlement_per_stack() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(8_833),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(8_833),
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
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: 1_049,
        buff_id: 6_280_511,
        team_type: 2,
        owner_alive: true,
        amount: 3,
        key: SubscriptionKey::new(
            EventKind::RoundStart,
            crate::engine::skill::rule::DefinitionKey::new(213, "Dot"),
        ),
        act_type: "Dot".to_owned(),
        effect_time: 101,
        effect_condition: 0,
        args: vec![1, AttrId::CurrentHp.id(), 30],
        raw: "213#1#100#30".to_owned(),
    };

    let ops = layered_damage_rule_ops(
        &managers,
        &pool,
        &mut RoundDeterminism::default(),
        &subscriber,
    );

    assert_eq!(ops.len(), 6);
    for pair in ops.chunks_exact(2) {
        assert!(matches!(
            pair,
            [
                RuleOp::BuffFeatureMarker {
                    effect_type,
                    buff_act_id: 213,
                    ..
                },
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    amount: 264,
                    ..
                })))
            ] if *effect_type == EffectType::Dot as i32
        ));
    }
}

#[test]
fn corrode_uses_current_hp_damage_capped_by_the_casters_max_hp() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
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
                current_hp: Some(5_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(5_000),
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
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 301_801,
        team_type: 2,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(202, "Dot"),
        ),
        act_type: "Dot".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![1, AttrId::CurrentHp.id(), 150, 0, AttrId::Hp.id(), 150],
        raw: "202#1#100#150#0#101#150".to_owned(),
    };

    assert!(matches!(
        damage_rule_op(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber,
        ),
        Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss { amount: 150, .. }
        ))))
    ));
}

#[test]
fn a_burn_immune_holder_takes_no_burn_damage_and_keeps_locked_stacks() {
    crate::test_support::init_config();
    let burn = |uid: i64| BuffInfo {
        uid: Some(uid),
        buff_id: Some(4150001),
        from_uid: Some(10),
        layer: Some(4),
        ..Default::default()
    };
    let entity = |uid: i64, buffs: Vec<BuffInfo>| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1_000),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(1_000),
            attack: Some(500),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    vec![
                        burn(20),
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(30940001),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                ),
                entity(11, vec![burn(22)]),
                entity(
                    12,
                    vec![
                        burn(23),
                        BuffInfo {
                            uid: Some(24),
                            buff_id: Some(123100181),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                ),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let subscriber = |owner_uid: i64, buff_uid: i64| BuffActSubscriber {
        owner_uid,
        source_uid: 10,
        buff_uid,
        buff_id: 4150001,
        team_type: 1,
        owner_alive: true,
        amount: 4,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(726, "Burn"),
        ),
        act_type: "Burn".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![150, 102, 40],
        raw: "726#150#102#40".to_owned(),
    };
    let mut determinism = RoundDeterminism::default();

    assert!(damage_rule_ops(&managers, &pool, &mut determinism, &subscriber(10, 20)).is_empty());
    assert!(settlement_rule_op(&managers, &subscriber(10, 20)).is_none());

    // A holder with only the stack lock still takes Burn damage.
    assert!(!damage_rule_ops(&managers, &pool, &mut determinism, &subscriber(12, 23)).is_empty());
    assert!(settlement_rule_op(&managers, &subscriber(12, 23)).is_none());

    assert!(!damage_rule_ops(&managers, &pool, &mut determinism, &subscriber(11, 22)).is_empty());
    assert!(matches!(
        settlement_rule_op(&managers, &subscriber(11, 22)),
        Some(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::SetAmount(BuffSetAmount {
                amount: BuffAmount::Layer(2),
                ..
            })
        )))
    ));
}

#[test]
fn settlement_emits_a_command_without_mutating_the_buff() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30620111),
                    layer: Some(4),
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
        buff_id: 30620111,
        team_type: 1,
        owner_alive: true,
        amount: 4,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(1048, "Radiance"),
        ),
        act_type: "Radiance".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: Vec::new(),
        raw: "1048".to_owned(),
    };

    assert!(matches!(
        settlement_rule_op(&managers, &subscriber),
        Some(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::SetAmount(BuffSetAmount {
                amount: BuffAmount::Layer(2),
                ..
            })
        )))
    ));
    assert_eq!(managers.buff.snapshot(10, 20).unwrap().layer, Some(4));
}

#[test]
fn dot_applies_source_and_target_genesis_modifiers_without_a_hit_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        attack: Some(1_000),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(1),
                            buff_id: Some(30980152),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(3),
                            buff_id: Some(31050145),
                            from_uid: Some(20),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    buffs: vec![BuffInfo {
                        uid: Some(4),
                        buff_id: Some(31050111),
                        layer: Some(22),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(500),
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(30980126),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        team_type: 2,
        owner_alive: true,
        amount: 3,
        key: SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(726, "Burn"),
        ),
        act_type: "Burn".to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![0, 102, 40],
        raw: "726#0#102#40".to_owned(),
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let output = damage_rule_op(&managers, &pool, &mut determinism, &subscriber).unwrap();
    let mut events = EventBus::default();

    assert_eq!(managers.hp.current(-1), 1_000);
    execute_rule_op(&mut managers, &mut events, output).unwrap();

    assert_eq!(managers.hp.current(-1), 901);
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::HpLost { amount: 99, .. })
    ));
    assert!(events.is_empty());
}

#[test]
fn caster_scaled_dot_keeps_its_grant_snapshot_and_commits_hp_loss() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(1_000),
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
                team_type: Some(2),
                current_hp: Some(5_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(5_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
    };
    let dot_uid = managers
        .execute_buff(BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 10,
                target_uid: -1,
                buff_id: 5051,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    managers
        .execute_buff(BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 2240001,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    assert_eq!(managers.origin_attribute(10, AttrId::Attack), 2_000);

    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: dot_uid,
        buff_id: 5051,
        team_type: 2,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
        ),
        act_type: "Dot".to_owned(),
        effect_time: 210,
        effect_condition: 0,
        args: vec![0, AttrId::Attack.id(), 1_000],
        raw: "203#0#102#1000".to_owned(),
    };
    let pool = TargetPool::from_fight(&fight);
    let output = damage_rule_op(
        &managers,
        &pool,
        &mut RoundDeterminism::default(),
        &subscriber,
    )
    .unwrap();
    let mut events = EventBus::default();

    execute_rule_op(&mut managers, &mut events, output).unwrap();

    assert_eq!(managers.hp.current(-1), 4_000);
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::HpLost {
            source_uid: 10,
            target_uid: -1,
            amount: 1_000,
            ..
        })
    ));
}

#[test]
fn holder_scaled_dot_reads_current_hp_when_the_action_triggers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
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
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
    };
    let buff_uid = managers
        .execute_buff(BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 117361042,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    managers
        .execute_hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: 10,
            target_uid: 10,
            amount: 200,
            config_effect: 0,
            hurt: None,
        }))
        .unwrap();

    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid,
        buff_id: 117361042,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
        ),
        act_type: "Dot".to_owned(),
        effect_time: 210,
        effect_condition: 0,
        args: vec![1, AttrId::CurrentHp.id(), 100],
        raw: "203#1#100#100".to_owned(),
    };
    let output = damage_rule_op(
        &managers,
        &TargetPool::from_fight(&fight),
        &mut RoundDeterminism::default(),
        &subscriber,
    )
    .unwrap();

    execute_rule_op(&mut managers, &mut EventBus::default(), output).unwrap();

    assert_eq!(managers.hp.current(10), 720);
}
