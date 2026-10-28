use super::*;

#[test]
fn manager_owned_passive_skills_preserve_configured_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![900, 100, 900],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let entity = pool.entity(10).unwrap();

    assert_eq!(
        entity_skill_owners(entity, &pool, &managers),
        vec![(10, 900), (10, 100)]
    );
}

#[test]
fn conduit_energy_reactions_delay_only_equipped_psychube_passives() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    passive_skill: vec![31440141, 31450141],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(1),
                    passive_skill: vec![437011],
                    trial_equip: Some(EquipRecord {
                        equip_id: Some(1570),
                        refine_lv: Some(0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    passive_skill: vec![437311],
                    equips: vec![EquipRecord {
                        equip_id: Some(1573),
                        refine_lv: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let subscribers = for_compiled_event(&pool, &managers, &catalog, EventKind::ConduitActivated)
        .unwrap()
        .skills;

    assert_eq!(
        subscribers
            .iter()
            .find(|subscriber| subscriber.skill_id == 31440141)
            .unwrap()
            .key
            .timing,
        crate::engine::event::subscription::ReactionTiming::Immediate
    );
    assert_eq!(
        subscribers
            .iter()
            .find(|subscriber| subscriber.skill_id == 31450141)
            .unwrap()
            .key
            .timing,
        crate::engine::event::subscription::ReactionTiming::Immediate
    );
    assert_eq!(
        subscribers
            .iter()
            .find(|subscriber| subscriber.skill_id == 437311)
            .unwrap()
            .key
            .timing,
        crate::engine::event::subscription::ReactionTiming::AfterAction
    );
    assert_eq!(
        subscribers
            .iter()
            .find(|subscriber| subscriber.skill_id == 437011)
            .unwrap()
            .key
            .timing,
        crate::engine::event::subscription::ReactionTiming::AfterAction
    );

    managers.entity.update(FightEntityInfo {
        uid: Some(11),
        current_hp: Some(1),
        passive_skill: vec![437311],
        ..Default::default()
    });
    assert_eq!(
        managers.entity.passive_source(11, 437311),
        crate::engine::entity::passive::PassiveSourceKind::Psychube
    );
}

#[test]
fn recreated_buff_keeps_owner_reaction_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                team_type: Some(2),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31090112),
                        from_uid: Some(10),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31090131),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    crate::engine::mechanic::shell::execute(
        &mut managers,
        crate::engine::mechanic::shell::ShellCommand::Retrieve {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::BuffAct,
                key: DefinitionKey::new(871, "ShellDebuff"),
            },
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            amount: 1,
        },
    )
    .unwrap();

    let shell_reactions = for_active_buffs(&managers, EventKind::ShellRetrieved)
        .into_iter()
        .filter_map(|subscriber| {
            matches!(
                buff_act::subscriber_kind(&subscriber),
                Some(
                    buff_act::registry::BuffActKind::ShellProcess
                        | buff_act::registry::BuffActKind::ShellLock
                )
            )
            .then_some((subscriber.owner_uid, subscriber.key.definition.opcode))
        })
        .collect::<Vec<_>>();

    assert_eq!(shell_reactions, vec![(10, 869), (-1, 873)]);
}

#[test]
fn reserve_promotion_does_not_reorder_buff_reaction_owners() {
    crate::test_support::init_config();
    let entity = |uid, current_hp, position, buff_uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(current_hp),
        team_type: Some(1),
        position: Some(position),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31090131),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 0, 1, 20)],
            sub_entitys: vec![entity(11, 100, -1, 21)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.entity.promote_reserves(&managers.hp).len(), 1);
    let owners = for_active_buffs(&managers, EventKind::ShellRetrieved)
        .into_iter()
        .filter(|subscriber| {
            buff_act::subscriber_kind(subscriber)
                == Some(buff_act::registry::BuffActKind::ShellLock)
        })
        .map(|subscriber| subscriber.owner_uid)
        .collect::<Vec<_>>();

    assert_eq!(owners, vec![10, 11]);
}

#[test]
fn reachable_missing_skill_is_not_a_silent_empty_subscription() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![999_999],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        for_compiled_event(
            &pool,
            &managers,
            &SkillEffectCatalog::default(),
            EventKind::SkillAction,
        ),
        Err(SubscriberError::MissingSkill {
            owner_uid: 10,
            skill_id: 999_999,
        })
    );
}

#[test]
fn setup_discovery_keeps_configured_owners_for_exact_condition_resolution() {
    crate::test_support::init_config();
    let fight = Fight {
        episode_id: Some(90_001_601),
        defender: Some(FightTeam {
            entitys: [(-2, 900_016_101), (-3, 900_016_102)]
                .into_iter()
                .map(|(uid, model_id)| FightEntityInfo {
                    uid: Some(uid),
                    model_id: Some(model_id),
                    team_type: Some(2),
                    current_hp: Some(1),
                    passive_skill: vec![811453],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let subscribers = for_compiled_setup_stage(
        &pool,
        &managers,
        &catalog,
        SetupStage::RoundTransitionStart,
        1,
    )
    .unwrap();

    assert_eq!(
        subscribers
            .iter()
            .filter(|subscriber| subscriber.skill_id == 811453)
            .map(|subscriber| (subscriber.owner_uid, subscriber.slot_index))
            .collect::<Vec<_>>(),
        vec![(-2, 0), (-2, 1), (-3, 0), (-3, 1)]
    );
}

#[test]
fn active_field_self_skills_are_runtime_subscribers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 100051,
                    duration: 1,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();

    assert!(active_skills(&pool, &managers).contains(&(10, 308801821)));
}

#[test]
fn virtual_owner_buff_passives_are_runtime_subscribers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(20),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(116370321),
                    uid: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: crate::engine::manager::emitter::UID,
                buff_id: 116370321,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();

    assert!(
        active_skills(&pool, &managers)
            .contains(&(crate::engine::manager::emitter::UID, 116370321))
    );
    assert!(!active_skills(&pool, &managers).contains(&(20, 116370321)));
}

#[test]
fn projected_passive_links_do_not_become_intrinsic_subscribers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                passive_skill: vec![71012],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut projected = fight;
    projected.attacker.as_mut().unwrap().entitys[0]
        .passive_skill
        .insert(0, 71013);
    let pool = TargetPool::from_fight(&projected);

    let skills = entity_skill_owners(pool.entity(10).unwrap(), &pool, &managers);

    assert!(skills.contains(&(10, 71012)));
    assert!(!skills.contains(&(10, 71013)));
}

#[test]
fn field_subscribers_precede_projected_buff_links() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(308801211),
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
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 100051,
                    duration: 1,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let mut projected = fight;
    projected.attacker.as_mut().unwrap().entitys[0]
        .passive_skill
        .push(308802011);
    let pool = TargetPool::from_fight(&projected);

    assert_eq!(
        entity_skill_owners(pool.entity(10).unwrap(), &pool, &managers),
        vec![(10, 308801821), (10, 308802011)]
    );
}

#[test]
fn transformed_identity_subscribes_through_its_death_event_only() {
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 30111005,
                parameters: [1000, 0],
            },
        })
        .unwrap();

    let subscribers = for_compiled_event(
        &pool.runtime_view(&managers),
        &managers,
        crate::engine::skill::effect::catalog::global(),
        EventKind::EntityDied,
    )
    .unwrap();

    assert!(
        subscribers
            .skills
            .iter()
            .any(|subscriber| subscriber.owner_uid == -1 && subscriber.skill_id == 530002746)
    );

    managers.hp.resync_current(-1, 0);
    let subscribers = for_compiled_event(
        &pool.runtime_view_including(&managers, Some(-1)),
        &managers,
        crate::engine::skill::effect::catalog::global(),
        EventKind::EntityDied,
    )
    .unwrap();
    assert!(
        subscribers
            .skills
            .iter()
            .any(|subscriber| subscriber.owner_uid == -1 && subscriber.skill_id == 530002746)
    );

    let subscribers = for_compiled_event(
        &pool.runtime_view(&managers),
        &managers,
        crate::engine::skill::effect::catalog::global(),
        EventKind::BuffRemoved,
    )
    .unwrap();
    assert!(
        subscribers
            .skills
            .iter()
            .all(|subscriber| subscriber.owner_uid != -1)
    );
}
