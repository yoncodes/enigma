use super::*;

#[test]
fn provoke_redirects_single_target_attacks_to_the_buff_source() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(229103),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let attack = TargetContext {
        runtime_target_uid: 11,
        active_skill_is_attack: true,
        ..Default::default()
    };

    assert_eq!(
        TargetResolver::resolve_action_targets(
            &TargetRequest {
                code: 201,
                raw: Vec::new(),
            },
            1,
            -1,
            &pool,
            &mut RoundDeterminism::default(),
            None,
            attack,
        ),
        vec![10]
    );
    assert_eq!(
        TargetResolver::resolve_action_targets(
            &TargetRequest {
                code: 201,
                raw: Vec::new(),
            },
            1,
            -1,
            &pool,
            &mut RoundDeterminism::default(),
            None,
            TargetContext {
                active_skill_is_attack: false,
                ..attack
            },
        ),
        vec![11]
    );
}

#[test]
fn provoke_makes_its_source_the_primary_multi_target_attack_target() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2), entity_at(12, 3)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(229103),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        TargetResolver::resolve_action_targets(
            &TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
            109380001,
            -1,
            &TargetPool::from_fight(&fight),
            &mut RoundDeterminism::default(),
            None,
            TargetContext {
                runtime_target_uid: 11,
                active_skill_is_attack: true,
                ..Default::default()
            },
        ),
        vec![10, 11, 12]
    );
}

#[test]
fn taunt_and_mock_taunt_each_redirect_single_target_attacks() {
    init_config();
    for (buff_id, expected_uid) in [(5042, 10), (2220011, 11)] {
        let mut first = entity_at(10, 1);
        let mut second = entity_at(11, 2);
        let holder = if expected_uid == 10 {
            &mut first
        } else {
            &mut second
        };
        holder.buffs.push(BuffInfo {
            uid: Some(20),
            buff_id: Some(buff_id),
            from_uid: Some(expected_uid),
            ..Default::default()
        });
        let pool = TargetPool::from_fight(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![first, second, entity_at(12, 3)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity_at(-1, 1)],
                ..Default::default()
            }),
            ..Default::default()
        });

        assert_eq!(
            TargetResolver::resolve_action_targets(
                &TargetRequest {
                    code: 201,
                    raw: Vec::new(),
                },
                1,
                -1,
                &pool,
                &mut RoundDeterminism::default(),
                None,
                TargetContext {
                    runtime_target_uid: 12,
                    active_skill_is_attack: true,
                    ..Default::default()
                },
            ),
            vec![expected_uid]
        );
    }
}

#[test]
fn runtime_view_sees_buffs_and_deaths_applied_after_fight_setup() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let initial_pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: -1,
                buff_id: 229103,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    managers.hp.lose(11, 100, 0);

    let pool = initial_pool.runtime_view(&managers);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([SkillTargetChoice {
        skill_id: 1,
        source_uid: -1,
        target_code: 201,
        targets: vec![11],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    }]);
    let targets = TargetResolver::resolve_action_targets(
        &TargetRequest {
            code: 201,
            raw: Vec::new(),
        },
        1,
        -1,
        &pool,
        &mut determinism,
        Some(&managers),
        TargetContext {
            active_skill_is_attack: true,
            ..Default::default()
        },
    );

    assert_eq!(targets, vec![10]);
    assert!(pool.entity(11).is_none());
}

#[test]
fn runtime_view_uses_the_committed_transformed_identity() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(30111001),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let base = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 30111005,
                parameters: [1000, 0],
            },
        })
        .unwrap();

    let transformed = base.runtime_view(&managers);

    assert_eq!(transformed.entity(-1).unwrap().model_id, 30111005);
}
