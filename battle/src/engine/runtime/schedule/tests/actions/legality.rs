use super::*;

#[test]
fn forbid_discards_non_ultimate_support_incantations_without_playing_them() {
    init_config();
    assert!(buff_act::registry::has_destination(406, "Forbid", &[]));
    assert!(!buff_act::wire::find(406, "Forbid").unwrap().has_output());
    assert!(buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::Debuff as i32,
        false,
    ));
    assert!(buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::Buff as i32,
        false,
    ));
    assert!(buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::CounterSpell as i32,
        false,
    ));
    assert!(buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::Heal as i32,
        false,
    ));
    assert!(!buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::RealityDamage as i32,
        false,
    ));
    assert!(!buff_act::forbid::blocks(
        crate::engine::skill::effect::catalog::SkillEffectTag::Buff as i32,
        true,
    ));

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(4062),
                    from_uid: Some(-1),
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
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(100),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(200),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(200),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_effect_tag(
        100,
        crate::engine::skill::effect::catalog::SkillEffectTag::Debuff as i32,
    );
    catalog.insert_effect_tag(
        200,
        crate::engine::skill::effect::catalog::SkillEffectTag::RealityDamage as i32,
    );

    let result = run_player_commands(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RoundCommand::PlayCard {
            card_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(managers.card.hand().len(), 2);
    assert!(
        managers
            .card
            .hand()
            .iter()
            .all(|card| card.skill_id == Some(200))
    );
    assert_eq!(managers.ex_point.get(10), 0);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
    );

    run_player_commands(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RoundCommand::PlayCard {
            card_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(managers.card.hand().len(), 1);
    assert_eq!(managers.card.hand()[0].skill_id, Some(200));
}

#[test]
fn disarm_blocks_basic_attack_incantations_through_card_legality() {
    init_config();
    assert!(buff_act::registry::has_destination(405, "Disarm", &[]));
    assert_eq!(
        buff_act::wire::find(405, "Disarm")
            .unwrap()
            .markers(buff_act::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::Disarm as i32]
    );

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(4051),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_effect_tag(
        100,
        crate::engine::skill::effect::catalog::SkillEffectTag::RealityDamage as i32,
    );
    catalog.insert_effect_tag(
        200,
        crate::engine::skill::effect::catalog::SkillEffectTag::Buff as i32,
    );

    assert!(card_skill_is_blocked(&managers, &catalog, 10, 100));
    assert!(!card_skill_is_blocked(&managers, &catalog, 10, 200));
}

#[test]
fn seal_blocks_only_ultimate_incantations() {
    init_config();
    assert_eq!(
        buff_act::registry::destination(407, "Seal", &[]),
        Some(buff_act::registry::BuffActDestination::StateConsumer)
    );

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(4071),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = crate::engine::skill::effect::catalog::global();

    assert!(card_skill_is_blocked(&managers, catalog, 10, 30480131));
    assert!(!card_skill_is_blocked(&managers, catalog, 10, 30480111));
}

#[test]
fn incapacitating_control_buffs_block_card_actions() {
    init_config();
    for (buff_id, kind) in [
        (4011, buff_act::registry::BuffActKind::Dizzy),
        (4020, buff_act::registry::BuffActKind::Petrified),
        (4040, buff_act::registry::BuffActKind::Frozen),
    ] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(buff_id),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::default();
        assert!(managers.buff.has_buff_act_kind(10, kind));
        assert!(card_skill_is_blocked(&managers, &catalog, 10, 1));
    }
}

#[test]
fn channeling_blocks_regular_and_ultimate_card_actions() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(30830131),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = crate::engine::skill::effect::catalog::global();

    for skill_id in [30830121, 30830131] {
        assert!(card_skill_is_blocked(&managers, catalog, 10, skill_id));
    }
}

#[test]
fn contract_channeling_blocks_binder_and_bound_card_actions() {
    init_config();
    for (buff_id, kind) in [
        (
            31_000_141,
            buff_act::registry::BuffActKind::ContractCastChannel,
        ),
        (31_000_151, buff_act::registry::BuffActKind::NoneCastChannel),
    ] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(buff_id),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);

        assert!(managers.buff.has_buff_act_kind(10, kind));
        assert!(card_skill_is_blocked(
            &managers,
            &SkillEffectCatalog::default(),
            10,
            100,
        ));
    }
}

#[test]
fn unsupported_contract_channel_shapes_do_not_block_card_actions() {
    init_config();
    for (buff_id, raw, kind) in [
        (
            31_000_141,
            "836#1#-1#31000151#31000171",
            buff_act::registry::BuffActKind::ContractCastChannel,
        ),
        (
            31_000_151,
            "837#1",
            buff_act::registry::BuffActKind::NoneCastChannel,
        ),
    ] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(buff_id),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers.buff.replace_buff_features_for_test(10, raw);

        assert!(!managers.buff.has_buff_act_kind(10, kind));
        assert!(!card_skill_is_blocked(
            &managers,
            &SkillEffectCatalog::default(),
            10,
            100,
        ));
    }
}

#[test]
fn sleep_blocks_every_active_card_action() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(4031),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(card_skill_is_blocked(
        &BattleManagers::seeded(&fight),
        &SkillEffectCatalog::default(),
        10,
        100,
    ));
}
