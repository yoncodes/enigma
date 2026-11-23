use super::*;

#[test]
fn buff_layers_enchant_owned_basic_cards_before_allied_cards() {
    use crate::engine::runtime::determinism::{HandCardChoice, RoundDeterminism};
    use sonettobuf::{BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3142),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31420001),
                        from_uid: Some(10),
                        layer: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3104),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |uid, skill_id, temp_card| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        card(10, 31420131, false),
        card(20, 31040121, false),
        card(10, 31420111, false),
        card(20, 31040111, true),
        card(20, 31040111, false),
    ]);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60285, "RemoveBuffToEnchant", vec![31420001, 10011, 1]);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hand_card_choices([HandCardChoice {
        skill_id: 31420151,
        opcode: 60285,
        hand_index: 1,
    }]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31420151,
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

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(consume))),
            RuleOp::Command(BattleCommand::Card(CardCommand::EnchantHand(enchant)))
        ] if consume.target_uid == 10
            && consume.selector == BuffSelector::ExactId(31420001)
            && consume.amount == 2
            && consume.depleted == DepletedBuff::Remove
            && enchant.indices == [2, 1]
            && enchant.enchant == EnchantedType::Ramona
            && enchant.duration == 1
            && enchant.team_type == 1
    ));
    assert_eq!(Handler::references(&behavior).buffs, vec![31420001]);

    for op in ops {
        match op {
            RuleOp::Command(BattleCommand::Buff(command)) => {
                managers.execute_buff(command).unwrap();
            }
            RuleOp::Command(BattleCommand::Card(command)) => {
                managers.card.execute_command(command).unwrap();
            }
            _ => unreachable!("opcode emits only buff and card commands"),
        }
    }
    assert_eq!(managers.buff.buff_id_amount(10, 31420001), 0);
    for index in [1, 2] {
        assert!(matches!(
            managers.card.hand()[index].enchants.as_slice(),
            [enchant]
                if enchant.enchant_id == Some(EnchantedType::Ramona.id())
                    && enchant.duration == Some(1)
        ));
    }
    for index in [0, 3, 4] {
        assert!(managers.card.hand()[index].enchants.is_empty());
    }
}

#[test]
fn special_temporary_card_uses_the_source_identity_as_reserve_id() {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60300, "AddSpTempCard2", vec![31446013]);

    for (source_uid, reserve_id) in [
        (10, Some(3149)),
        (crate::engine::fight::rules::ATTACKER_SIDE_UID, Some(0)),
        (999, None),
    ] {
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid,
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
        );

        let Some(reserve_id) = reserve_id else {
            assert!(ops.is_none());
            continue;
        };
        let ops = ops.unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))]
                if add.target_uid == 10
                    && add.skill_id == 31446013
                    && add.reserve_id == reserve_id
                    && add.team_type == 1
                    && add.kind
                        == crate::engine::manager::card::TemporaryCardKind::PrecastSkill
        ));
    }
    assert_eq!(Handler::references(&behavior).skills, vec![31446013]);
}

#[test]
fn generic_temporary_card_keeps_target_metadata_but_is_unowned() {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(50031, "AddSpTempCard", vec![31446013]);

    let ops = Handler::emit_ops(
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))]
            if add.target_uid == 10
                && add.skill_id == 31446013
                && add.reserve_id == 3149
                && add.team_type == 1
                && add.kind
                    == crate::engine::manager::card::TemporaryCardKind::GenericSkill
    ));
    assert_eq!(Handler::references(&behavior).skills, vec![31446013]);
}

#[test]
fn queued_skill_card_uses_committed_rank_threshold_and_next_queue_index() {
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(
        [30970111, 30970111, 30970111, 30970112]
            .into_iter()
            .map(|skill_id| CardInfo {
                uid: Some(10),
                skill_id: Some(skill_id),
                ..Default::default()
            })
            .collect(),
    );
    for _ in 0..4 {
        managers.card.play_card(0, None, None, None).unwrap();
    }
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let event = crate::engine::event::payload::BattleEvent::ActionQueueCommitted {
        team: 1,
        emitter_uid: 1,
        cards: vec![
            CardInfo {
                skill_id: Some(30970111),
                ..Default::default()
            },
            CardInfo {
                skill_id: Some(30970111),
                ..Default::default()
            },
            CardInfo {
                skill_id: Some(30970111),
                ..Default::default()
            },
            CardInfo {
                skill_id: Some(30970112),
                ..Default::default()
            },
        ],
        action_count: 4,
    };
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60070, "AddUseSkillCard"),
        vec![3, 30970171, 1, 1],
        vec![
            "3,5,7,99".into(),
            "30970171,30970172,30970173".into(),
            "1".into(),
            "1,30970111".into(),
        ],
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 30970161,
            transfer_count: 1,
            event: Some(&event),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(CardCommand::QueueUseCard(
            CardQueueUse {
                card_index: 5,
                card,
                team_type: 1,
                action: Some(SkillInvocation {
                    plan: SkillRequest {
                        source_uid: -1,
                        skill_id: 30970172,
                    },
                    mode: SkillExecutionMode::Active,
                    target: crate::engine::skill::action::SkillTarget::Configured,
                    ..
                }),
                ..
            }
        )))] if card.skill_id == Some(30970172) && card.hero_id == Some(1)
    ));
}

#[test]
fn around_change_rank_prepares_both_neighbor_mutations() {
    let behavior = ParsedBehavior::new(60075, "AroundChangeRank", vec![1, -1]);

    let ops = <Handler as BehaviorHandler>::collect_queue_preparation(6, &behavior).unwrap();

    let [RuleOp::Command(BattleCommand::Card(CardCommand::ChangeQueuedRanks { changes, .. }))] =
        ops.as_slice()
    else {
        panic!("AroundChangeRank must emit one owned card command")
    };
    assert_eq!(
        changes,
        &vec![
            crate::engine::manager::card::QueuedCardRankChange {
                card_index: 5,
                levels: 1,
            },
            crate::engine::manager::card::QueuedCardRankChange {
                card_index: 7,
                levels: -1,
            },
        ]
    );
}

#[test]
fn power_card_upgrade_spends_the_rank_cost_before_upgrading_one_owned_card() {
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3065),
                entity_type: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![30650211, 30650212, 30650213],
                skill_group2: vec![30650221, 30650222, 30650223],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(6),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: vec![
                    CardInfo {
                        uid: Some(10),
                        skill_id: Some(30650211),
                        ..Default::default()
                    },
                    CardInfo {
                        uid: Some(10),
                        skill_id: Some(30650221),
                        ..Default::default()
                    },
                ],
                draw_pile: Vec::new(),
                deck_num: 2,
            },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(50034, "ConsumePowerUpgradeSkillCard", vec![2, 3]);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    determinism.enqueue_hand_rank_choices([crate::engine::runtime::determinism::HandRankChoice {
        opcode: 50034,
        owner_uid: 10,
        hand_index: 1,
    }]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 30650243,
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

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: -2,
                ..
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::RankUpHand(
                HandCardRankUp {
                    owner_uid: 10,
                    hand_index: 1,
                    ..
                }
            )))
        ]
    ));
}

#[test]
fn card_level_change_mode_three_selects_visual_rightmost_owned_card() {
    use crate::engine::runtime::determinism::{HandRankChoice, RoundDeterminism};
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![100, 101, 102],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![200, 201],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |uid, skill_id, temp_card| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        card(10, 100, false),
        card(20, 200, false),
        card(10, 101, true),
        card(10, 999, false),
        card(10, 101, false),
        card(20, 200, false),
        card(10, 101, false),
        card(10, 100, true),
    ]);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(50011, "CardLevelChange", vec![3, 1, 1]);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hand_rank_choices([HandRankChoice {
        opcode: 50011,
        owner_uid: 10,
        hand_index: 0,
    }]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                owner_uid: 10,
                hand_index: 0,
                ..
            })
        ))]
    ));
    assert_eq!(determinism.take_hand_rank_choice(50011, 10, &[0]), Some(0));
    let mut expected = RoundDeterminism::default();
    assert_eq!(
        determinism.lua_random_index(4),
        expected.lua_random_index(4)
    );

    let mode_one = ParsedBehavior::new(50011, "CardLevelChange", vec![1, 1, 1]);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hand_rank_choices([HandRankChoice {
        opcode: 50011,
        owner_uid: 10,
        hand_index: 4,
    }]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
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
        &mode_one,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                owner_uid: 10,
                hand_index: 4,
                ..
            })
        ))]
    ));
}

#[test]
fn card_level_change_rejects_unsupported_mode_three_variants() {
    for args in [
        vec![3, 0, 1],
        vec![3, 2, 1],
        vec![3, 1, 0],
        vec![3, 1, 2],
        vec![2, 1, 1],
        vec![3, 1],
        vec![3, 1, 1, 1],
    ] {
        let behavior = ParsedBehavior::new(50011, "CardLevelChange", args);
        assert!(!supports_card_level_change(&behavior));
        assert!(!crate::engine::skill::behavior::is_supported(&behavior));
    }
}

#[test]
fn deck_top_rank_correct_selects_absolute_spellbox_positions() {
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![30650211, 30650212, 30650213],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![30870121, 30870122, 30870123],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: Vec::new(),
                draw_pile: vec![
                    CardInfo {
                        uid: Some(10),
                        skill_id: Some(30650211),
                        ..Default::default()
                    },
                    CardInfo {
                        uid: Some(20),
                        skill_id: Some(30870121),
                        ..Default::default()
                    },
                    CardInfo {
                        uid: Some(10),
                        skill_id: Some(30650211),
                        ..Default::default()
                    },
                    CardInfo {
                        uid: Some(20),
                        skill_id: Some(30870121),
                        ..Default::default()
                    },
                ],
                deck_num: 4,
            },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::new(60116, "CardDeckTopRankCorrect", vec![2, 3, 1]);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
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

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpDeckRange(crate::engine::manager::card::CardDeckRankUpRange {
                from: 2,
                to: 3,
                rank_delta: 1,
                ..
            })
        ))]
    ));
}

#[test]
fn deck_top_rank_correct_accepts_only_observed_argument_shape() {
    for args in [vec![1, 1, 1], vec![2, 3, 1], vec![4, 4, 1]] {
        let behavior = ParsedBehavior::new(60116, "CardDeckTopRankCorrect", args);
        assert!(supports_deck_top_rank_correct(&behavior));
        assert!(crate::engine::skill::behavior::is_supported(&behavior));
    }
    for args in [
        vec![0, 1, 1],
        vec![1, 2, 1],
        vec![2, 2, 1],
        vec![3, 3, 1],
        vec![3, 2, 1],
        vec![1, 1, 0],
        vec![1, 1, 2],
        vec![1, 1],
        vec![1, 1, 1, 1],
    ] {
        let behavior = ParsedBehavior::new(60116, "CardDeckTopRankCorrect", args);
        assert!(!supports_deck_top_rank_correct(&behavior));
        assert!(!crate::engine::skill::behavior::is_supported(&behavior));
    }
}
