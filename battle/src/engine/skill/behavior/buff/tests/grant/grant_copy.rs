use super::*;

#[test]
fn add_buff_marks_death_triggered_grants_for_manager_uid_ownership() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let event = crate::engine::event::payload::BattleEvent::EntityDied(
        crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -2,
        },
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 2,
            target_uid: -1,
            active_skill_id: 103374115,
            transfer_count: 1,
            event: Some(&event),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::new(1, "AddBuff", vec![103374115]),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantOnDeath(
            grant
        )))] if grant.buff_id == 103374115
    ));
}

#[test]
fn synchronization_buff_restarts_its_configured_action_cycle() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ex_point_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(100000, "EzioProps"),
    };
    managers
        .execute_ex_point(
            crate::engine::manager::ex_point::ExPointCommand::ConfigureSynchronization(
                crate::engine::manager::ex_point::ExPointConfigureSynchronization {
                    origin,
                    target_uid: 10,
                    definition: crate::engine::manager::ex_point::SynchronizationDefinition::new(
                        [312301323, 312301333, 312301343],
                        4,
                        100,
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
    managers
        .execute_ex_point(crate::engine::manager::ex_point::ExPointCommand::Set(
            crate::engine::manager::ex_point::ExPointSet {
                origin,
                source_uid: 10,
                target_uid: 10,
                value: 100,
                config_effect: 0,
                effect_type: 0,
            },
        ))
        .unwrap();
    managers
        .execute_ex_point(
            crate::engine::manager::ex_point::ExPointCommand::RecordSynchronizationAction(
                crate::engine::manager::ex_point::ExPointRecordSynchronizationAction {
                    origin,
                    target_uid: 10,
                    action_target_uid: -1,
                },
            ),
        )
        .unwrap();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 312301403,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::new(1, "AddBuff", vec![229100]),
    )
    .unwrap();

    let [
        RuleOp::Command(BattleCommand::ExPoint(restart)),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantStateful(grant))),
    ] = ops.as_slice()
    else {
        panic!("expected one cycle restart and one stateful grant");
    };
    assert_eq!(grant.params.as_deref(), Some("10000#4,0,0"));
    managers.execute_ex_point(*restart).unwrap();
    let added = managers
        .execute_buff(BuffCommand::GrantStateful(grant.clone()))
        .unwrap()
        .change
        .added
        .unwrap();
    assert_eq!(
        managers.ex_point.synchronization_progress(10),
        Some(crate::engine::manager::ex_point::SynchronizationProgress::default())
    );
    assert_eq!(managers.ex_point.get(10), 100);
    assert_eq!(added.buff.act_common_params.as_deref(), Some("10000#4,0,0"));
}

#[test]
fn source_relative_buff_keeps_emit_time_snapshot_across_queued_targets() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &sonettobuf::HeroExAttribute {
            cri_dmg: Some(2_000),
            ..Default::default()
        },
    );
    let behavior = ParsedBehavior::new(1, "AddBuff", vec![31440116]);
    let emit =
        |managers: &BattleManagers, target_uid, transfer_count, behavior: &ParsedBehavior| {
            let mut determinism = RoundDeterminism::default();
            let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
            let mut target = crate::engine::skill::target::TargetContext::default();
            let ops = super::super::super::super::rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid,
                    active_skill_id: 31441121,
                    transfer_count,
                    event: None,
                    managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                behavior,
            )
            .unwrap();
            let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
                panic!("expected one buff command");
            };
            command.clone()
        };

    let source_grant = emit(&managers, 10, 1, &behavior);
    let ally_grant = emit(&managers, 11, 1, &behavior);
    let repeated = emit(&managers, 11, 3, &behavior);
    assert!(matches!(
        repeated,
        BuffCommand::GrantSourceRelative(crate::engine::manager::buff::SourceRelativeBuffGrant {
            grant: BuffGrant { occurrences: 3, .. },
            ..
        })
    ));
    assert!(matches!(
        emit(
            &managers,
            11,
            1,
            &ParsedBehavior::new(2, "AddBuffPowerUse", vec![31440116]),
        ),
        BuffCommand::GrantSourceRelative(_)
    ));
    managers.execute_buff(source_grant).unwrap();
    assert_eq!(
        managers.origin_attribute(10, crate::engine::entity::attr::AttrId::CriticalDmg),
        2_600
    );

    let ally = managers.execute_buff(ally_grant).unwrap();
    assert_eq!(
        ally.change
            .added
            .as_ref()
            .unwrap()
            .buff
            .act_info
            .iter()
            .filter_map(|info| info.str_param.as_deref())
            .collect::<Vec<_>>(),
        ["203#600"]
    );
}

#[test]
fn layer_copy_uses_the_buff_limit_and_preserves_main_target_overflow() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(4150001),
                    layer: Some(6),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(4150001),
                        layer: Some(28),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60068, "AddBuffByBuffLayer", vec![4150001, 4150001, 1]);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        runtime_target_uid: -1,
        ..Default::default()
    };

    let main_ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 30940132,
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
    assert_eq!(target.buff_overflow_amount, 4);
    let [RuleOp::Command(BattleCommand::Buff(command))] = main_ops.as_slice() else {
        panic!("expected one buff command");
    };
    managers.execute_buff(command.clone()).unwrap();
    assert_eq!(managers.buff.buff_id_amount(-1, 4150001), 30);

    let secondary_ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -2,
            active_skill_id: 30940132,
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
    assert_eq!(target.buff_overflow_amount, 4);
    let [RuleOp::Command(BattleCommand::Buff(command))] = secondary_ops.as_slice() else {
        panic!("expected one buff command");
    };
    assert!(matches!(
        command,
        BuffCommand::Grant(BuffGrant {
            target_uid: -2,
            buff_id: 4150001,
            amount: Some(6),
            ..
        })
    ));
}

#[test]
fn layer_range_replaces_the_selected_buff_too() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31050111),
                        layer: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31050111),
                        layer: Some(6),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(31050142),
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
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60124,
            "AddBuffByBuffLayerRange",
        ),
        Vec::new(),
        vec![
            "31050111".into(),
            "31050141,31050142,31050143".into(),
            "5,15,25,100".into(),
        ],
    );
    assert!(super::super::supports_layer_range(&behavior));

    let malformed = ParsedBehavior::from_spec(
        behavior.spec.clone(),
        Vec::new(),
        vec![
            "31050111".into(),
            "31050141,31050142,31050143".into(),
            "5,25,15,100".into(),
        ],
    );
    assert!(!super::super::supports_layer_range(&malformed));

    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::super::rule_ops(
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

    let [RuleOp::Command(BattleCommand::Buff(BuffCommand::Replace(replace)))] = ops.as_slice()
    else {
        panic!("expected one atomic replacement");
    };
    assert_eq!(replace.sources, vec![BuffSelector::IdOrType(31050142)]);
    assert_eq!(replace.replacement_id_or_type, 31050142);
    assert_eq!(replace.child_uid_reservations, 1);
}
