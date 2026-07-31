use super::*;

#[test]
fn apple_rank_two_attack_heals_the_lowest_hp_ally() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3028),
                current_hp: Some(500),
                skill_group2: vec![30280121, 30280122, 30280123],
                attr: Some(HeroAttribute {
                    hp: Some(1_250),
                    attack: Some(231),
                    ..Default::default()
                }),
                base_attr: Some(HeroAttribute {
                    hp: Some(1_250),
                    attack: Some(231),
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
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
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
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(30280122),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 638);
}

#[test]
fn apple_ultimate_settles_death_before_its_after_damage_heal() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3028),
                current_hp: Some(500),
                ex_point: Some(5),
                ex_skill: Some(30280131),
                attr: Some(HeroAttribute {
                    hp: Some(1_250),
                    attack: Some(231),
                    ..Default::default()
                }),
                base_attr: Some(HeroAttribute {
                    hp: Some(1_250),
                    attack: Some(231),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(225),
                attr: Some(HeroAttribute {
                    hp: Some(225),
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
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(30280131),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let result = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    let step = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(30280131))
        .unwrap();
    assert_eq!(
        step.act_effect
            .iter()
            .filter_map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            sonettobuf::effect_type_enum::EffectType::Damage as i32,
            sonettobuf::effect_type_enum::EffectType::Dead as i32,
            sonettobuf::effect_type_enum::EffectType::Heal as i32,
        ]
    );
}

#[test]
fn decoded_card_commands_share_the_command_only_player_schedule() {
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
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [100, 200, 300]
                .into_iter()
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();

    let result = run_player_commands(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            RoundCommand::MoveCard {
                from_index: 0,
                to_index: 2,
            },
            RoundCommand::DissolveCard { card_index: 1 },
        ],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![200, 100]
    );
    assert_eq!(managers.ex_point.get(10), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
    );
    assert_eq!(
        steps[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    );
}

#[test]
fn ex_point_card_move_modifies_the_base_move_moxie() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3108),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(34_250_080),
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
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31_080_111, 31_080_121]
                .into_iter()
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();

    run_player_commands(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RoundCommand::MoveCard {
            from_index: 0,
            to_index: 1,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
}

#[test]
fn continuous_action_changes_ap_without_increasing_card_moxie() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31_130_113),
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

    assert_eq!(
        card_play_resource_delta(&managers, 10, true, false),
        Some(1)
    );
}

#[test]
fn named_boss_power_does_not_gain_card_moxie() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                power_infos: vec![PowerInfo {
                    power_id: Some(
                        crate::engine::manager::eureka::PowerType::ZongMaoBossEnergy.id(),
                    ),
                    num: Some(1),
                    max: Some(3),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(card_play_resource_delta(&managers, -1, true, false), None);
}

#[test]
fn rewritten_card_executes_for_its_resolved_caster() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 20]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ex_point: Some(0),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let source = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let resolved = CardInfo {
        uid: Some(20),
        skill_id: Some(200),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![source.clone()],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: Some(crate::engine::manager::card::CardPlayChoice {
                source: resolved.clone(),
                played: resolved,
            }),
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.card.played()[0].card.uid, source.uid);
    assert_eq!(managers.card.played()[0].caster_uid, 20);
    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.ex_point.get(20), 2);
}
