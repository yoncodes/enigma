use super::*;

#[test]
fn ai_actions_invalidate_dead_card_owners_and_grant_committed_basic_card_resource() {
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
                    ex_point: Some(5),
                    ex_skill: Some(999),
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            AiSkillChoice {
                source_uid: -1,
                skill_id: 100,
                target_uid: 10,
            },
            AiSkillChoice {
                source_uid: -2,
                skill_id: 999,
                target_uid: 10,
            },
        ],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 1);
    assert_eq!(managers.ex_point.get(-2), 0);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
    );
    assert_eq!(steps[0].act_effect[0].effect_num, Some(1));
    assert_eq!(steps[0].from_id, Some(-1));
    assert_eq!(steps[0].act_effect[0].config_effect, Some(0));
    assert_eq!(
        steps[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    );
    assert_eq!(steps[1].act_effect[0].effect_num, Some(1));
    let skill = steps.iter().find(|step| step.act_id == Some(999)).unwrap();
    assert_eq!(skill.card_index, Some(2));
    assert_eq!(skill.to_id, Some(10));
    assert!(skill.act_effect.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
            && effect.effect_num == Some(-5)
    }));
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

#[test]
fn action_queue_stops_when_the_configured_win_target_dies() {
    init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(hp),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        battle_id: Some(301110),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 30111001, 1), entity(-4, 30111002, 1_000)],
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
                key: DefinitionKey::new(40006, "MonsterChange"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::Transform {
                model_id: 30111005,
                parameters: [1_000, 0],
            },
        })
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        managers
            .entity
            .passive_skills(-1)
            .into_iter()
            .flatten()
            .copied(),
        std::iter::empty(),
    );
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(999, 1_000);

    run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            AiSkillChoice {
                source_uid: 10,
                skill_id: 999,
                target_uid: -1,
            },
            AiSkillChoice {
                source_uid: 10,
                skill_id: 999,
                target_uid: -4,
            },
        ],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.hp.current(-4), 1_000);
}
