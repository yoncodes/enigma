use super::*;

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
            )
            .with_timing(crate::engine::event::subscription::ReactionTiming::AfterSharedHit),
        }]
    );
}
