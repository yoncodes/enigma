use super::*;

#[test]
fn configured_circle_exposes_its_start_phase_skill() {
    crate::test_support::init_config();

    assert_eq!(self_skills(100051), vec![308801821]);
}

#[test]
fn magic_circle_attr_accepts_one_or_more_complete_valid_triples() {
    let behavior = |args| ParsedBehavior::new(60076, "MagicCircleAttr", args);

    assert!(Handler::supports(&behavior(vec![2, 214, 120])));
    assert!(Handler::supports(&behavior(vec![1, 301, 80, 2, 214, -120])));
}

#[test]
fn magic_circle_attr_rejects_incomplete_scope_or_attribute_triples() {
    let behavior = |args| ParsedBehavior::new(60076, "MagicCircleAttr", args);

    for args in [vec![2, 214], vec![3, 214, 120], vec![2, 999, 120]] {
        assert!(!Handler::supports(&behavior(args)));
    }
}

#[test]
fn magic_circle_attr_has_a_registry_owner_but_emits_no_runtime_operation() {
    let behavior = ParsedBehavior::new(60076, "MagicCircleAttr", vec![2, 214, 120]);
    let definition = crate::engine::skill::behavior::registry::find(&behavior).unwrap();
    assert!(definition.destination);
    assert!(crate::engine::skill::behavior::is_supported(&behavior));

    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
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
        ),
        Some(ops) if ops.is_empty()
    ));
}

#[test]
fn add_magic_circle_emits_a_config_derived_deploy_command() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::new(50019, "AddMagicCircle", vec![30001, 1]);

    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert!(matches!(
        deploy_rule_ops(&behavior, 10, 1, &BattleManagers::default(), &pool).as_deref(),
        Some([RuleOp::Command(BattleCommand::Field(FieldCommand {
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    ..
                },
                create_uid: 10,
                initial_level: 1,
                thresholds,
            },
            ..
        }))]) if thresholds.iter().any(|threshold| threshold.level == 2 && threshold.progress == 50)
    ));
}

#[test]
fn remove_magic_circle_only_removes_the_matching_active_field() {
    crate::test_support::init_config();
    let origin = crate::engine::manager::buff::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(50019, "AddMagicCircle"),
    };
    let mut managers = BattleManagers::default();
    managers
        .field
        .execute_command(FieldCommand {
            origin,
            team: 2,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 20009,
                    duration: 4,
                },
                create_uid: -3,
                initial_level: 0,
                thresholds: Vec::new(),
            },
        })
        .unwrap();

    assert!(
        remove_rule_ops(
            &ParsedBehavior::new(50021, "RemoveMagicCircleById", vec![20008]),
            2,
            &managers,
        )
        .unwrap()
        .is_empty()
    );
    assert!(matches!(
        remove_rule_ops(
            &ParsedBehavior::new(50021, "RemoveMagicCircleById", vec![20009]),
            2,
            &managers,
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::Field(FieldCommand {
            team: 2,
            operation: FieldOperation::Remove,
            ..
        }))]
    ));
}

#[test]
fn blood_domain_deployment_grants_its_configured_ally_buff() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![
                sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(11),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(50019, "AddMagicCircle", vec![100051]);

    let ops = deploy_rule_ops(&behavior, 10, 1, &BattleManagers::default(), &pool).unwrap();

    assert_eq!(ops.len(), 3);
    assert!(ops[..2].iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantArray {
            grant: BuffGrantChild {
                buff_id: 308801312,
                ..
            },
            owner: ArrayBuffOwner {
                team: 1,
                field_id: 100051,
                caster_uid: 10,
            },
        }))
    )));
    assert!(matches!(ops[2], RuleOp::Command(BattleCommand::Field(_))));
}

#[test]
fn removing_blood_domain_withdraws_its_linked_grants() {
    use crate::engine::{runtime::drain, skill::effect::SkillEffectCatalog};
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10, 11, 12, 13]
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .to_vec(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [308801821], []);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let deploy = ParsedBehavior::new(50019, "AddMagicCircle", vec![100051]);
    let remove = ParsedBehavior::new(50021, "RemoveMagicCircleById", vec![100051]);
    assert!(super::Handler::supports(&remove));
    let ops = deploy_rule_ops(&deploy, 10, 1, &managers, &pool).unwrap();
    drain::run(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();

    assert!(managers.field.get(1).is_some());
    let grants =
        [10, 11, 12, 13].map(|uid| (uid, managers.buff.buff_id_uid(uid, 308801312).unwrap()));
    let ops = remove_rule_ops(&remove, 1, &managers).unwrap();
    drain::run(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();

    assert!(managers.field.get(1).is_none());
    for (uid, buff_uid) in grants {
        assert!(managers.buff.snapshot(uid, buff_uid).is_none());
    }
}

#[test]
fn qi_panorama_deployment_grants_its_linked_emitter_buffs_after_the_field() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(50019, "AddMagicCircle", vec![42004]);

    let ops = deploy_rule_ops(&behavior, 10, 1, &BattleManagers::default(), &pool).unwrap();
    let definition = crate::engine::skill::behavior::registry::find(&behavior).unwrap();

    assert!(matches!(ops[0], RuleOp::Command(BattleCommand::Field(_))));
    assert_eq!((definition.output_owner_for)(&behavior, &ops[0], 0), None);
    assert_eq!(
        ops[1..]
            .iter()
            .enumerate()
            .map(|(index, op)| match op {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(grant))) => {
                    assert_eq!(grant.target_uid, emitter::UID);
                    assert_eq!(
                        (definition.output_owner_for)(&behavior, op, index + 1),
                        Some(OutputOwner::SetupParent)
                    );
                    grant.buff_id
                }
                _ => panic!("linked magic-circle output must be an emitter buff"),
            })
            .collect::<Vec<_>>(),
        [31410601, 31412602, 31410603, 31410604]
    );
}

#[test]
fn wangqi_update_emits_one_config_derived_level_transaction() {
    crate::test_support::init_config();
    let origin = crate::engine::manager::buff::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(50019, "AddMagicCircle"),
    };
    let mut managers = BattleManagers::default();
    managers
        .field
        .execute_command(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 42003,
                    duration: -1,
                },
                create_uid: 7,
                initial_level: 3,
                thresholds: Vec::new(),
            },
        })
        .unwrap();

    let behavior = ParsedBehavior::new(60270, "UpdateWangQiMagicCircle", vec![1]);
    assert!(matches!(
        update_wangqi_rule_ops(&behavior, 7, 1, &managers).as_deref(),
        Some([RuleOp::Command(BattleCommand::MagicCircleLevel(
            MagicCircleLevelCommand {
                expected_field_id: 42003,
                replacement: FieldDefinition {
                    field_id: 42004,
                    duration: 4,
                },
                level: 0,
                removed_buffs,
                added_buffs,
                ..
            }
        ))]) if removed_buffs == &vec![31410601, 31412602, 31410603]
            && added_buffs == &vec![31410601, 31412602, 31410603, 31410604]
    ));
    assert!(crate::engine::skill::behavior::is_supported(&behavior));
    assert!(!crate::engine::skill::behavior::is_supported(
        &ParsedBehavior::new(60270, "UpdateWangQiMagicCircle", vec![2],)
    ));
}
