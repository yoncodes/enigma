use super::*;

#[test]
fn setup_initializes_owned_card_state_without_publishing_a_change() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::default();

    let changes = manager
        .execute_command(CardCommand::Setup(CardSetup {
            hand: vec![card(100)],
            draw_pile: vec![card(200)],
            deck_num: 30,
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::Setup);
    assert!(changes.events().is_empty());
    assert_eq!(manager.hand()[0].skill_id, Some(100));
    assert_eq!(manager.draw_pile()[0].skill_id, Some(200));
    assert_eq!(manager.deck_num(), 30);
}

#[test]
fn hand_rank_change_mutates_one_registered_card_and_records_its_one_based_index() {
    crate::test_support::init_config();
    let mut manager = CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(30650221),
        ..Default::default()
    }]);
    manager.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                skill_group1: vec![30650221, 30650222, 30650223],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute_command(CardCommand::RankUpHand(HandCardRankUp {
            origin: ORIGIN,
            owner_uid: 10,
            hand_index: 0,
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::HandRankChanged);
    assert_eq!(manager.hand()[0].skill_id, Some(30650222));
    assert!(matches!(
        changes.rank_results.as_slice(),
        [CardRankResult::Changed(change)]
            if change.card_index == 1 && change.card.skill_id == Some(30650222)
    ));
}

#[test]
fn deck_rank_range_uses_absolute_top_positions_and_commits_atomically() {
    crate::test_support::init_config();
    let card = |uid, skill_id, temp_card| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    };
    let hand = vec![card(10, 30650211, false)];
    let draw_pile = vec![
        card(10, 30650211, false),
        card(20, 30870121, false),
        card(10, 30650221, false),
        card(10, 30650211, false),
    ];
    let mut manager = CardManager::with_draw_pile(hand.clone(), draw_pile);
    manager.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![
                sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    skill_group1: vec![30650211, 30650212, 30650213],
                    skill_group2: vec![30650221, 30650222, 30650223],
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(20),
                    skill_group1: vec![30870121, 30870122, 30870123],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });

    let changes = manager
        .execute_command(CardCommand::RankUpDeckRange(CardDeckRankUpRange {
            origin: ORIGIN,
            from: 2,
            to: 3,
            rank_delta: 1,
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::DeckTopRanksChanged);
    assert_eq!(
        manager
            .draw_pile()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![30650211, 30870122, 30650222, 30650211]
    );
    assert_eq!(manager.hand(), hand);
    assert!(matches!(
        changes.rank_results.as_slice(),
        [CardRankResult::Changed(first), CardRankResult::Changed(second)]
            if first.card_index == 2 && second.card_index == 3
    ));

    let mut capped = CardManager::with_draw_pile(
        Vec::new(),
        vec![card(10, 30650211, false), card(10, 30650213, false)],
    );
    capped.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                skill_group1: vec![30650211, 30650212, 30650213],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let before = capped.draw_pile().to_vec();
    let result = capped.execute_command(CardCommand::RankUpDeckRange(CardDeckRankUpRange {
        origin: ORIGIN,
        from: 1,
        to: 2,
        rank_delta: 1,
    }));
    assert_eq!(result, Err(CardCommandError::InvalidCommand));
    assert_eq!(capped.draw_pile(), before);
}

#[test]
fn move_and_dissolve_commit_through_the_card_owner() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card(100), card(200), card(300)]);

    let moved = manager
        .execute_command(CardCommand::Move {
            origin: ORIGIN,
            from_index: 0,
            to_index: 2,
        })
        .unwrap();
    assert_eq!(moved.kind, CardChangeKind::Moved);
    assert_eq!(
        moved
            .after
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![200, 300, 100]
    );
    assert!(moved.operation.is_none());

    let dissolved = manager
        .execute_command(CardCommand::Dissolve {
            origin: ORIGIN,
            card_index: 1,
        })
        .unwrap();
    assert_eq!(dissolved.kind, CardChangeKind::Dissolved);
    assert_eq!(
        dissolved
            .after
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![200, 100]
    );
    assert!(matches!(
        dissolved.operation,
        Some(CardChange::CardsPush { ref cards, team_type: 1 }) if cards == &dissolved.after
    ));
}

#[test]
fn temporary_card_and_energy_changes_share_one_command_path() {
    let mut manager = CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        card_effect: Some(1),
        energy: Some(1),
        ..Default::default()
    }]);

    let added = manager
        .execute_command(CardCommand::AddTemporary(CardAddTemporary {
            origin: ORIGIN,
            target_uid: 10,
            skill_id: 999,
            hero_id: None,
            reserve_id: 0,
            team_type: 1,
            kind: TemporaryCardKind::ConfiguredSkill,
        }))
        .unwrap();
    assert_eq!(added.kind, CardChangeKind::TemporaryAdded);
    assert_eq!(added.added.as_ref().unwrap().skill_id, Some(999));
    assert_eq!(added.added.as_ref().unwrap().uid, Some(10));
    assert_eq!(added.added.as_ref().unwrap().temp_card, Some(true));
    assert_eq!(
        added.added.as_ref().unwrap().card_type,
        Some(sonettobuf::card_info::CardType::Skill3 as i32)
    );
    assert!(matches!(
        added.operation,
        Some(CardChange::SpCardAdd {
            target_uid: 10,
            skill_id: 999,
            reserve_id: 0,
            team_type: 1,
        })
    ));
    assert_eq!(added.events().len(), 1);

    let energy = manager
        .execute_command(CardCommand::ChangeBasicEnergy(CardEnergyChange {
            origin: ORIGIN,
            delta: 2,
            count: 1,
        }))
        .unwrap();
    assert_eq!(energy.after[0].energy, Some(3));
    assert_eq!(
        energy.events()[0].kind(),
        crate::engine::event::kind::EventKind::CardChanged
    );

    let cleared = manager
        .execute_command(CardCommand::ClearEnergy { origin: ORIGIN })
        .unwrap();
    assert!(cleared.after.iter().all(|card| card.energy == Some(0)));

    let changed = manager
        .execute_command(CardCommand::ChangeToTemporary(CardChangeToTemporary {
            origin: ORIGIN,
            index: 0,
            skill_id: 777,
            target_uid: 10,
            reserve: "configured".to_owned(),
            team_type: 1,
        }))
        .unwrap();
    assert_eq!(changed.kind, CardChangeKind::TemporaryChanged);
    assert_eq!(changed.after[0].skill_id, Some(777));
    assert!(matches!(
        changed.operation,
        Some(CardChange::ChangeToTemp {
            target_uid: 10,
            ref reserve_str,
            team_type: 1,
        }) if reserve_str == "configured"
    ));

    let expired = manager
        .execute_command(CardCommand::ExpireTemporary { origin: ORIGIN })
        .unwrap();
    assert_eq!(expired.expired_indices, [0, 1]);
    assert!(manager.hand().is_empty());
}

#[test]
fn generic_temporary_card_is_unowned_but_retains_operation_metadata() {
    let mut manager = CardManager::default();
    let changes = manager
        .execute_command(CardCommand::AddTemporary(CardAddTemporary {
            origin: ORIGIN,
            target_uid: 10,
            skill_id: 999,
            hero_id: None,
            reserve_id: 3149,
            team_type: 1,
            kind: TemporaryCardKind::GenericSkill,
        }))
        .unwrap();
    let card = changes.added.as_ref().unwrap();

    assert_eq!(changes.kind, CardChangeKind::GenericTemporaryAdded);
    assert_eq!(card.uid, Some(0));
    assert_eq!(card.skill_id, Some(999));
    assert_eq!(card.hero_id, Some(0));
    assert_eq!(card.card_type, Some(0));
    assert_eq!(card.temp_card, Some(true));
    assert!(matches!(
        changes.operation,
        Some(CardChange::SpCardAdd {
            target_uid: 10,
            skill_id: 999,
            reserve_id: 3149,
            team_type: 1,
        })
    ));
}

#[test]
fn dissolve_reports_its_own_snapshot_after_an_unrelated_card_operation() {
    let mut manager = CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    }]);
    manager
        .execute_command(CardCommand::AddGenerated(CardAddGenerated {
            origin: ORIGIN,
            target_uid: 10,
            skill_id: 200,
            hero_id: None,
            team_type: 0,
        }))
        .unwrap();

    let dissolved = manager
        .execute_command(CardCommand::Dissolve {
            origin: ORIGIN,
            card_index: 0,
        })
        .unwrap();

    assert!(matches!(
        dissolved.operation,
        Some(CardChange::CardsPush { ref cards, team_type: 1 })
            if cards == &dissolved.after && cards[0].skill_id == Some(200)
    ));
}

#[test]
fn owner_skill_replacement_is_a_typed_card_command() {
    let mut manager = CardManager::with_draw_pile(
        vec![CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            ..Default::default()
        }],
        vec![CardInfo {
            uid: Some(10),
            skill_id: Some(110),
            ..Default::default()
        }],
    );

    let changes = manager
        .execute_command(CardCommand::ReplaceOwnerSkills(CardReplaceOwnerSkills {
            origin: ORIGIN,
            owner_uid: 10,
            base_group1: vec![100],
            base_group2: vec![110],
            replacement_group1: vec![200],
            replacement_group2: vec![210],
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::OwnerSkillsReplaced);
    assert_eq!(changes.after[0].skill_id, Some(200));
    assert_eq!(manager.draw_pile()[0].skill_id, Some(210));
}

#[test]
fn generated_card_keeps_add_hand_semantics() {
    let mut manager = CardManager::default();

    let added = manager
        .execute_command(CardCommand::AddGenerated(CardAddGenerated {
            origin: ORIGIN,
            target_uid: 10,
            skill_id: 999,
            hero_id: None,
            team_type: 0,
        }))
        .unwrap();

    assert_eq!(added.kind, CardChangeKind::GeneratedAdded);
    assert_eq!(manager.hand()[0].skill_id, Some(999));
    assert_eq!(manager.hand()[0].temp_card, Some(false));
    assert!(matches!(
        added.operation,
        Some(CardChange::AddHand { target_uid: 10, card })
            if card.skill_id == Some(999)
    ));
}

#[test]
fn owned_generated_card_keeps_its_owner_and_team_semantics() {
    let mut manager = CardManager::default();

    let added = manager
        .execute_command(CardCommand::AddGenerated(CardAddGenerated {
            origin: ORIGIN,
            target_uid: 10,
            skill_id: 999,
            hero_id: Some(3124),
            team_type: 1,
        }))
        .unwrap();

    assert_eq!(added.kind, CardChangeKind::OwnedGeneratedAdded);
    assert_eq!(manager.hand()[0].hero_id, Some(3124));
    assert_eq!(manager.hand()[0].temp_card, Some(false));
    assert_eq!(manager.hand()[0].card_type, None);
    assert!(matches!(
        added.operation,
        Some(CardChange::OwnedAddHand { card, team_type: 1 })
            if card.hero_id == Some(3124) && card.skill_id == Some(999)
    ));
}

#[test]
fn card_setup_rules_commit_enchants_and_temporary_lifecycle_through_the_manager() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card(100), card(200), card(300)]);

    let enchanted = manager
        .execute_command(CardCommand::EnchantHand(CardEnchantHand {
            origin: ORIGIN,
            indices: vec![2, 0],
            enchant: EnchantedType::Burn,
            duration: -1,
            team_type: 1,
        }))
        .unwrap();
    assert_eq!(enchanted.kind, CardChangeKind::Enchanted);
    assert!(matches!(
        enchanted.operation,
        Some(CardChange::Enchant {
            ref indices,
            ref cards,
            team_type: 1,
        }) if indices == &[2, 0]
            && cards.iter().all(|card| card.enchants.iter().any(|enchant| {
                enchant.enchant_id == Some(EnchantedType::Burn.id())
                    && enchant.duration == Some(-1)
            }))
    ));

    manager
        .execute_command(CardCommand::MarkTemporary(CardMarkTemporary {
            origin: ORIGIN,
            indices: vec![0, 1, 2],
            team_type: 1,
            config_effect: 50023,
        }))
        .unwrap();
    assert!(
        manager
            .hand()
            .iter()
            .all(|card| card.temp_card == Some(true))
    );
    manager
        .execute_command(CardCommand::AllocateEnergy(CardEnergyAllocation {
            origin: ORIGIN,
            energies: vec![1, 2, 3],
        }))
        .unwrap();
    manager
        .execute_command(CardCommand::AddPrecast(CardAddPrecast {
            origin: ORIGIN,
            card: crate::engine::manager::card::precast_card(10, 900),
        }))
        .unwrap();

    let expired = manager
        .execute_command(CardCommand::ExpireTemporary { origin: ORIGIN })
        .unwrap();
    assert_eq!(expired.kind, CardChangeKind::TemporaryExpired);
    assert_eq!(expired.expired_indices, [0, 1, 2]);
    assert_eq!(manager.hand().len(), 1);
    assert_eq!(manager.hand()[0].skill_id, Some(900));
}

#[test]
fn precast_cards_cannot_enter_the_draw_pile_or_crystal_lane_as_normal_cards() {
    let mut manager = CardManager::default();
    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };

    assert_eq!(
        manager.execute_command(CardCommand::Setup(CardSetup {
            hand: Vec::new(),
            draw_pile: vec![crate::engine::manager::card::precast_card(10, 900)],
            deck_num: 1,
        })),
        Err(CardCommandError::InvalidCommand)
    );
    assert_eq!(
        manager.execute_command(CardCommand::AddCrystal(CardAddCrystal {
            origin: ORIGIN,
            card: normal,
            rank_group: Vec::new(),
        })),
        Err(CardCommandError::InvalidCommand)
    );

    manager
        .execute_command(CardCommand::AddCrystal(CardAddCrystal {
            origin: ORIGIN,
            card: crate::engine::manager::card::precast_card(10, 900),
            rank_group: vec![900, 901],
        }))
        .unwrap();
    assert_eq!(manager.normal_hand_len(), 0);
    assert_eq!(manager.deck_num(), 0);

    let played = manager.play_card(0, None, None, None).unwrap();
    assert_eq!(
        manager.rank_up_played(played.card_index, 1, true),
        Some(901)
    );
}
