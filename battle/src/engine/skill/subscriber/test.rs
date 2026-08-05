use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::{
    condition::{ParsedCondition, ParsedConditionKind, buff::BuffConditionMode, none::NoneMode},
    effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
    target::{TargetPool, TargetRequest},
};

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
fn transformed_identity_remains_an_event_subscription_owner() {
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
        &TargetPool::default(),
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
}

#[test]
fn finds_only_rules_subscribed_to_the_published_event() {
    let mut event_slot = SkillEffectSlot::new(
        ParsedBehavior::new(20002, "AddExPoint", vec![1]),
        TargetRequest::self_only(),
    );
    event_slot.conditions = vec![ParsedCondition {
        opcode: 208,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillAction),
        raw_args: Vec::new(),
    }];
    let mut static_slot = SkillEffectSlot::new(
        ParsedBehavior::new(20002, "AddExPoint", vec![1]),
        TargetRequest::self_only(),
    );
    static_slot.conditions = vec![ParsedCondition {
        opcode: 19004,
        type_name: "HasBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![1],
        },
        raw_args: vec!["1".to_owned()],
    }];
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![event_slot, static_slot],
    });
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let entity = pool.entity(10).unwrap();

    assert_eq!(
        for_entity(entity, &catalog, EventKind::SkillAction),
        vec![SkillSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: None,
            key: SubscriptionKey::at_phase(
                EventKind::SkillAction,
                DefinitionKey::new(208, "None"),
                Some(crate::engine::skill::action::SkillPhase::AfterDamage),
            ),
        }]
    );
    assert!(for_entity(entity, &catalog, EventKind::RoundEnd).is_empty());
}

#[test]
fn discovers_configured_late_round_start_passive() {
    crate::test_support::init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![31340141],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    assert_eq!(
        for_round_start_priority(&pool, &BattleManagers::seeded(&fight), &catalog, 1,),
        vec![SkillSubscriber {
            owner_uid: 10,
            skill_id: 31340141,
            slot_index: Some(1),
            key: SubscriptionKey::new(EventKind::RoundStart, DefinitionKey::new(103, "None"),),
        }]
    );
}

#[test]
fn reserve_entities_do_not_publish_lifecycle_events() {
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(20002, "AddExPoint", vec![1]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 208,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillAction),
        raw_args: Vec::new(),
    }];
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![100],
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(1),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        for_event(&pool, &managers, &catalog, EventKind::SkillAction).skills,
        vec![SkillSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: None,
            key: SubscriptionKey::at_phase(
                EventKind::SkillAction,
                DefinitionKey::new(208, "None"),
                Some(crate::engine::skill::action::SkillPhase::AfterDamage),
            ),
        }]
    );
}

#[test]
fn setup_stages_do_not_discover_reserve_passives_before_promotion() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(1),
                passive_skill: vec![2_240_002],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let battle_start =
        for_compiled_setup_stage(&pool, &managers, &catalog, SetupStage::BattleStart, 0).unwrap();
    let enter_fight =
        for_compiled_setup_stage(&pool, &managers, &catalog, SetupStage::EnterFight, 0).unwrap();

    assert!(battle_start.is_empty());
    assert!(enter_fight.is_empty());
}

#[test]
fn setup_stages_discover_assist_boss_passives() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(999_999),
                passive_skill: vec![1_249_102],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(100),
                passive_skill: vec![811_453],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let subscribers =
        for_compiled_setup_stage(&pool, &managers, &catalog, SetupStage::EnterFight, 0).unwrap();

    assert!(subscribers.iter().any(|subscriber| {
        subscriber.owner_uid == -1
            && subscriber.skill_id == 1_249_102
            && subscriber.key == DefinitionKey::new(5, "EnterFight")
    }));
    let assist = subscribers
        .iter()
        .position(|subscriber| subscriber.owner_uid == -1)
        .unwrap();
    let defender = subscribers
        .iter()
        .position(|subscriber| subscriber.owner_uid == -2)
        .unwrap();
    assert!(assist < defender);
}

#[test]
fn skill_cast_discovers_assist_boss_attack_passives() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(999_999),
                passive_skill: vec![12_720_012],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let subscribers = for_compiled_event(&pool, &managers, &catalog, EventKind::SkillCast).unwrap();

    assert!(subscribers.skills.iter().any(|subscriber| {
        subscriber.owner_uid == -1
            && subscriber.skill_id == 12_720_012
            && subscriber.key.definition == DefinitionKey::new(2081, "None")
    }));
}

#[test]
fn discovers_event_passives_linked_by_active_buffs() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(31200124),
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
    let managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31200142]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 22211,
        type_name: "BeAttacked".to_owned(),
        kind: ParsedConditionKind::TargetAttacked,
        raw_args: Vec::new(),
    }];
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31200222,
        slots: vec![slot],
    });

    assert_eq!(
        for_event(&pool, &managers, &catalog, EventKind::TargetAttacked).skills,
        vec![SkillSubscriber {
            owner_uid: 10,
            skill_id: 31200222,
            slot_index: None,
            key: SubscriptionKey::new(
                EventKind::TargetAttacked,
                DefinitionKey::new(22211, "BeAttacked"),
            ),
        }]
    );
}

#[test]
fn discovers_active_buff_acts_from_effect_time() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(8112651),
                    from_uid: Some(11),
                    count: Some(3),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let subscribers = for_active_buffs(&managers, EventKind::SkillAction);

    assert_eq!(subscribers.len(), 1, "{subscribers:#?}");
    assert!(subscribers.iter().all(|subscriber| {
        subscriber.owner_uid == 10
            && subscriber.source_uid == 11
            && subscriber.buff_uid == 2
            && subscriber.buff_id == 8112651
            && subscriber.team_type == 1
            && subscriber.amount == 3
    }));
    let add_to_target = subscribers
        .iter()
        .find(|subscriber| subscriber.key.definition.opcode == 518)
        .unwrap();
    assert_eq!(
        add_to_target.key,
        SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(518, "AddToTarget"),
            Some(crate::engine::skill::action::SkillPhase::Immediate),
        )
    );
    assert_eq!(add_to_target.act_type, "AddToTarget");
    assert_eq!(add_to_target.effect_time, 201);
    assert_eq!(add_to_target.args, vec![1, 8112652]);
    let completed = for_active_buffs(&managers, EventKind::SkillCast);
    assert_eq!(completed.len(), 2, "{completed:#?}");
    assert!(completed.iter().any(|subscriber| {
        subscriber.key
            == SubscriptionKey::at_phase_and_publication(
                EventKind::SkillCast,
                DefinitionKey::new(503, "AddToTarget"),
                None,
                crate::engine::event::subscription::PublicationPhase::BeforePublish,
            )
            && subscriber.args == vec![1, 4011]
    }));
    assert!(completed.iter().any(|subscriber| {
        subscriber.key
            == SubscriptionKey::new(EventKind::SkillCast, DefinitionKey::new(827, "Bullet"))
            && subscriber.effect_time == 208
            && subscriber.effect_condition == 3
            && subscriber.args.is_empty()
    }));
    assert!(for_active_buffs(&managers, EventKind::RoundEnd).is_empty());

    managers.buff.delete(10, 2);
    assert!(for_active_buffs(&managers, EventKind::SkillAction).is_empty());
}

#[test]
fn exact_team_tags_compile_runtime_and_setup_subscriptions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(6_270_501),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(31_340_003),
                        from_uid: Some(10),
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

    assert_eq!(for_active_buffs(&managers, EventKind::HpLost).len(), 1);
    assert_eq!(for_active_buffs(&managers, EventKind::BuffAdded).len(), 1);
    assert_eq!(
        for_active_buffs(&managers, EventKind::ActionQueueCommitted).len(),
        1
    );
    let battle_start = buff_acts_for_setup_stage(&managers, SetupStage::BattleStart, 0);
    assert_eq!(battle_start.len(), 3);
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(875, "EmitterTag") })
    );
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(953, "BloodPoolTag") })
    );
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(1052, "HeatScaleTag") })
    );
}

#[test]
fn discovers_damage_calculation_buff_acts_without_publishing_a_fake_runtime_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(99_998),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(31080144),
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

    let subscribers = for_damage_calculation(&managers);

    assert_eq!(subscribers.len(), 2);
    assert!(subscribers.iter().all(|subscriber| {
        subscriber.key.event == EventKind::DamageCalculation
            && subscriber.key.definition.opcode == 884
            && buff_act::subscriber_kind(subscriber)
                == Some(buff_act::registry::BuffActKind::AddBuffAfterAttack)
    }));
}
