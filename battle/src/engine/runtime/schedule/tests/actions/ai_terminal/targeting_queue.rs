use super::*;

#[test]
fn ai_self_target_skills_ignore_the_selected_opponent() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(100),
                team_type: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [116330112], []);

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -2,
            skill_id: 116330112,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find(|step| step.act_id == Some(116330112))
        .unwrap();
    assert_eq!((skill.from_id, skill.to_id), (Some(-2), Some(-2)));
    assert_eq!(skill.act_effect[0].target_id, Some(-2));
}

#[test]
fn ai_actions_see_provoke_added_after_the_target_pool_was_built() {
    init_config();
    let entity = |uid, team_type, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        position: Some(position),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            defense: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1), entity(11, 1, 2), entity(12, 1, 3)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(999, 1_000);
    catalog.insert_logic_target(999, 202);

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 999,
            target_uid: 11,
        }],
        &[],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps.iter().find(|step| step.act_id == Some(999)).unwrap();
    assert_eq!(skill.to_id, Some(10));
}

#[test]
fn missing_allied_monster_is_resummoned_after_active_skill_damage() {
    init_config();
    let entity = |uid, team_type, model_id, position, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        model_id: Some(model_id),
        position: Some(position),
        current_hp: Some(current_hp),
        skill_group1: (model_id == 103370111)
            .then_some(103374111)
            .into_iter()
            .collect(),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 3149, 1, 100_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, 103370111, 1, 100_000),
                entity(-2, 2, 103370112, 2, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [103374111], []);

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 103374111,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find_map(|step| find_skill(step, 103374111))
        .unwrap();
    let effect_types = skill
        .act_effect
        .iter()
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    let damage = effect_types
        .iter()
        .position(|kind| *kind == sonettobuf::effect_type_enum::EffectType::Damage as i32)
        .expect("active skill should project damage");
    let summon = effect_types
        .iter()
        .position(|kind| *kind == sonettobuf::effect_type_enum::EffectType::Summon as i32)
        .expect("missing ally should project a summon");

    assert!(damage < summon);
}

#[test]
fn ai_action_phase_consumes_the_current_queue() {
    let fight = Fight::default();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-2),
                    skill_id: Some(100),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();

    run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        &[],
    )
    .unwrap();

    assert!(managers.card.ai_queue().is_empty());
}

#[test]
fn ai_actions_credit_a_promoted_replacement_when_the_card_is_invalidated() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    team_type: Some(-1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    team_type: Some(-1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();
    let promotions = [crate::engine::fight::reserve::Promotion {
        defeated_uid: -1,
        entering_uid: -2,
        position: 1,
        team_type: 2,
        entering: FightEntityInfo::default(),
    }];

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 100,
            target_uid: 10,
        }],
        &promotions,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 0);
    assert_eq!(managers.ex_point.get(-2), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
    );
    assert_eq!(steps[0].act_effect[0].effect_num, Some(1));
    assert_eq!(steps[0].from_id, Some(-1));
    assert_eq!(steps[0].act_effect[0].config_effect, Some(0));
    let invalid = steps
        .iter()
        .position(|step| step.from_id == Some(-1))
        .unwrap();
    let reward = steps
        .iter()
        .position(|step| {
            step.act_effect[0].effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
        })
        .unwrap();
    assert!(invalid < reward);
    assert_eq!(steps[reward].act_effect[0].effect_num, Some(1));
    assert_eq!(steps[reward].act_effect[0].target_id, Some(-2));
}

#[test]
fn ai_actions_do_not_credit_a_defeated_combatant_replaced_by_a_summon() {
    init_config();
    let entity = |uid, position, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1, 100), entity(-2, 2, 0), entity(-3, 3, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 30111003,
                position: 2,
            },
        })
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        managers
            .entity
            .passive_skills(-4)
            .into_iter()
            .flatten()
            .copied(),
        std::iter::empty(),
    );

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            AiSkillChoice {
                source_uid: -3,
                skill_id: 100,
                target_uid: 10,
            },
            AiSkillChoice {
                source_uid: -2,
                skill_id: 100,
                target_uid: 10,
            },
        ],
        &[],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-2), 0);
    assert_eq!(managers.ex_point.get(-3), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let rewards = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .filter(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
        })
        .map(|effect| effect.target_id)
        .collect::<Vec<_>>();
    assert_eq!(rewards, [Some(-3)]);
}

#[test]
fn ai_actions_invalidate_a_queued_ultimate_after_its_resource_is_lost() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(40231731),
                team_type: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid: -1,
            target_uid: -1,
            delta: -5,
            config_effect: 30001,
            effect_type: 0,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40231731,
        slots: Vec::new(),
    });

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 40231731,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 0);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].from_id, Some(-1));
    assert!(!steps.iter().any(|step| step.act_id == Some(40231731)));
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
    );
    assert_eq!(steps[0].act_effect[0].effect_num, Some(1));
}

#[test]
fn entity_card_cleanup_groups_dead_owners_in_one_owned_phase() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![
                    CardInfo {
                        uid: Some(-1),
                        skill_id: Some(100),
                        ..Default::default()
                    },
                    CardInfo {
                        uid: Some(-2),
                        skill_id: Some(200),
                        ..Default::default()
                    },
                ],
            },
        ))
        .unwrap();

    let result = run_entity_card_cleanup(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [-1, -2],
    )
    .unwrap();

    assert!(managers.card.ai_queue().is_empty());
    assert!(matches!(
        result.frames[0].owner,
        FrameOwner::RoundPhase(RoundPhase::EntityCardCleanup)
    ));
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 1);
    let removals = steps[0]
        .act_effect
        .iter()
        .map(|effect| {
            let nested = effect.fight_step.as_ref().unwrap();
            assert_eq!(
                effect.effect_type,
                Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32)
            );
            let removal = &nested.act_effect[0];
            assert_eq!(
                removal.effect_type,
                Some(sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32)
            );
            removal.target_id.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(removals, vec![-1, -2]);
}
