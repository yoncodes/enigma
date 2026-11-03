use super::*;

#[test]
fn selected_precast_uses_the_normal_add_hand_change_without_entering_hand_capacity() {
    let mut manager = CardManager::default();
    let changes = manager
        .execute_command(CardCommand::AddSelectedPrecast(CardAddPrecast {
            origin: ORIGIN,
            card: crate::engine::manager::card::selected_precast_card(10, 3149, 900),
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::GeneratedAdded);
    assert_eq!(manager.normal_hand_len(), 0);
    assert!(matches!(
        changes.operation,
        Some(CardChange::AddHand {
            target_uid: 10,
            ref card,
        }) if card.temp_card == Some(true) && card.hero_id == Some(3149)
    ));

    let current_round = manager
        .execute_command(CardCommand::ExpireTemporary { origin: ORIGIN })
        .unwrap();
    assert!(current_round.expired_indices.is_empty());
    manager.begin_round();
    let next_round = manager
        .execute_command(CardCommand::ExpireTemporary { origin: ORIGIN })
        .unwrap();
    assert_eq!(next_round.expired_indices, [0]);
    assert!(manager.hand().is_empty());
    assert_eq!(
        manager.draw_pile(),
        &[sonettobuf::CardInfo {
            temp_card: Some(false),
            ..crate::engine::manager::card::selected_precast_card(10, 3149, 900)
        }]
    );
}

#[test]
fn invalidated_play_discards_normal_cards_and_restores_ultimates() {
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(900),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card.clone()]);
    manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    let restored = manager
        .execute_command(CardCommand::InvalidatePlayed(CardInvalidatePlayed {
            origin: ORIGIN,
            card_index: 1,
            restore: true,
        }))
        .unwrap();

    assert!(manager.played().is_empty());
    assert_eq!(manager.hand(), &[card]);
    assert!(matches!(
        restored.operation,
        Some(CardChange::AddHand { target_uid: 10, .. })
    ));

    manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    let discarded = manager
        .execute_command(CardCommand::InvalidatePlayed(CardInvalidatePlayed {
            origin: ORIGIN,
            card_index: 1,
            restore: false,
        }))
        .unwrap();

    assert!(manager.hand().is_empty());
    assert!(discarded.operation.is_none());
}

#[test]
fn invalidating_ai_owner_reports_original_queue_indices() {
    let mut manager = CardManager::default();
    manager
        .execute_command(CardCommand::SetAiQueue(CardSetAiQueue {
            origin: ORIGIN,
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
                CardInfo {
                    uid: Some(-1),
                    skill_id: Some(300),
                    ..Default::default()
                },
            ],
        }))
        .unwrap();

    let changes = manager
        .execute_command(CardCommand::InvalidateAiOwner(CardInvalidateAiOwner {
            origin: ORIGIN,
            owner_uid: -1,
            team_type: 2,
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::AiCardsInvalidated);
    assert_eq!(
        changes
            .ai_invalidations
            .iter()
            .map(|invalidated| (invalidated.card_index, invalidated.card.skill_id))
            .collect::<Vec<_>>(),
        vec![(1, Some(100)), (3, Some(300))]
    );
    assert_eq!(manager.ai_queue().len(), 1);
    assert_eq!(manager.ai_queue()[0].uid, Some(-2));
}

#[test]
fn cloth_cards_add_universal_and_redeal_without_touching_special_cards() {
    crate::test_support::init_config();
    let mut manager = CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            card_effect: Some(2),
            ..Default::default()
        },
        CardInfo {
            uid: Some(0),
            skill_id: Some(118353040),
            temp_card: Some(true),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(308801322),
            ..Default::default()
        },
    ]);
    manager.rank_up.insert((20, 200), 201);

    let added = manager
        .execute_command(CardCommand::AddUniversal(CardAddUniversal {
            origin: ORIGIN,
            count: 1,
            rank: 1,
        }))
        .unwrap();
    assert_eq!(added.kind, CardChangeKind::UniversalAdded);
    assert_eq!(added.after.last().unwrap().skill_id, Some(30_000_001));

    let redealt = manager
        .execute_command(CardCommand::ApplyRedealKeepRanks(CardRedealKeepRanks {
            origin: ORIGIN,
            replacements: vec![CardInfo {
                uid: Some(20),
                skill_id: Some(200),
                ..Default::default()
            }],
        }))
        .unwrap();
    assert_eq!(redealt.kind, CardChangeKind::RedealtKeepRanks);
    assert_eq!(redealt.after[0].skill_id, Some(201));
    assert_eq!(redealt.after[1].skill_id, Some(118353040));
    assert_eq!(redealt.after[2].skill_id, Some(308801322));
    assert_eq!(redealt.after[3].skill_id, Some(30_000_001));
}

#[test]
fn rank_one_universal_rejects_rank_two_targets() {
    let target = |rank| CardInfo {
        uid: Some(10),
        skill_id: Some(if rank == 1 { 100 } else { 101 }),
        card_effect: Some(rank),
        ..Default::default()
    };
    let universal = CardInfo {
        uid: Some(0),
        skill_id: Some(super::super::super::UniversalCardSkill::RankOne.id()),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![universal.clone(), target(1)]);
    manager.rank_up.insert((10, 100), 101);

    let combined = manager
        .execute_command(CardCommand::UseUniversal(CardUseUniversal {
            origin: ORIGIN,
            universal_index: 0,
            target_index: 1,
        }))
        .unwrap();
    assert_eq!(combined.after.len(), 1);
    assert_eq!(combined.after[0].skill_id, Some(101));

    let mut manager = CardManager::new(vec![target(2), universal]);
    manager.rank_up.insert((10, 101), 102);
    assert_eq!(
        manager.execute_command(CardCommand::UseUniversal(CardUseUniversal {
            origin: ORIGIN,
            universal_index: 1,
            target_index: 0,
        })),
        Err(CardCommandError::InvalidCommand)
    );
    assert_eq!(manager.hand().len(), 2);
}

#[test]
fn effect_consumption_removes_only_planned_owner_skill_cards() {
    let mut manager = CardManager::new(vec![
        CardInfo {
            uid: Some(10),
            skill_id: Some(100),
            card_effect: Some(1),
            ..Default::default()
        },
        CardInfo {
            uid: Some(20),
            skill_id: Some(200),
            card_effect: Some(1),
            ..Default::default()
        },
        CardInfo {
            uid: Some(10),
            skill_id: Some(101),
            card_effect: Some(2),
            ..Default::default()
        },
    ]);
    manager.rank_up.insert((10, 100), 101);

    let changes = manager
        .execute_command(CardCommand::ConsumeForEffect(CardConsumeForEffect {
            origin: ORIGIN,
            owner_uid: 10,
            indices: vec![0, 2],
            kind: CardConsumptionKind::Generic,
        }))
        .unwrap();

    assert_eq!(changes.kind, CardChangeKind::ConsumedForEffect);
    assert_eq!(changes.after.len(), 1);
    assert_eq!(changes.after[0].uid, Some(20));
    assert_eq!(changes.consumed_indices, vec![0, 2]);
    assert_eq!(changes.events().len(), 1);
}

#[test]
fn invalid_temporary_card_does_not_mutate_the_hand() {
    let mut manager = CardManager::default();
    let result = manager.execute_command(CardCommand::AddTemporary(CardAddTemporary {
        origin: ORIGIN,
        target_uid: 10,
        skill_id: 0,
        hero_id: None,
        reserve_id: 0,
        team_type: 1,
        kind: TemporaryCardKind::ConfiguredSkill,
    }));

    assert_eq!(result, Err(CardCommandError::InvalidCommand));
    assert!(manager.hand().is_empty());
}

#[test]
fn draw_compose_and_play_commit_owned_card_state() {
    let card = || CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut manager = CardManager::with_draw_pile(vec![card()], vec![card()]);
    manager.rank_up.insert((10, 100), 101);

    let drawn = manager
        .execute_command(CardCommand::Draw(CardDraw {
            origin: ORIGIN,
            count: 1,
        }))
        .unwrap();
    assert_eq!(drawn.drawn.len(), 1);

    let composed = manager
        .execute_command(CardCommand::ComposeAdjacent { origin: ORIGIN })
        .unwrap();
    assert_eq!(composed.composed_owners, vec![10]);
    assert_eq!(composed.after[0].skill_id, Some(101));

    let played = manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    assert_eq!(played.played.unwrap().skill_id, 101);
    assert!(played.after.is_empty());
}

#[test]
fn captured_choice_resolves_by_card_identity_not_stale_hand_index() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card(100)]);

    let played = manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 7,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: Some(CardPlayChoice {
                source: card(100),
                played: card(101),
            }),
            recorded_skill: None,
        }))
        .unwrap();

    assert_eq!(played.played.unwrap().skill_id, 101);
    assert!(manager.hand().is_empty());
}

#[test]
fn visible_team_card_index_is_valid_after_hand_operations() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card(100), card(200)]);
    manager.set_team_cards(vec![card(900)]);
    manager
        .execute_command(CardCommand::Move {
            origin: ORIGIN,
            from_index: 0,
            to_index: 1,
        })
        .unwrap();

    let played = manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 2,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    assert_eq!(played.played.unwrap().skill_id, 900);
    assert_eq!(manager.hand(), &[card(200), card(100)]);
    assert!(manager.team_cards().is_empty());
}

#[test]
fn action_queue_snapshot_keeps_unplayed_team_cards() {
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![card(100)]);
    manager.set_team_cards(vec![card(900), card(901)]);
    manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 1,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    let committed = manager
        .execute_command(CardCommand::CommitActionQueue {
            team: 1,
            emitter_uid: 0,
            include_device_action: false,
        })
        .unwrap();

    assert_eq!(committed.after, vec![card(100), card(901)]);
}

#[test]
fn action_queue_snapshot_uses_the_selected_skill() {
    let mut manager = CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    }]);
    manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: Some(900),
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    let committed = manager
        .execute_command(CardCommand::CommitActionQueue {
            team: 1,
            emitter_uid: 0,
            include_device_action: false,
        })
        .unwrap();

    assert_eq!(committed.action_queue.unwrap().cards[0].skill_id, Some(900));
}

#[test]
fn action_queue_snapshot_appends_neutral_device_action() {
    let played = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![played.clone()]);
    manager
        .execute_command(CardCommand::Play(CardPlay {
            origin: ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();

    let committed = manager
        .execute_command(CardCommand::CommitActionQueue {
            team: 1,
            emitter_uid: 0,
            include_device_action: true,
        })
        .unwrap();
    let cards = committed.action_queue.unwrap().cards;

    assert_eq!(cards[0], played);
    assert_eq!(
        cards[1],
        CardInfo {
            uid: Some(0),
            skill_id: Some(0),
            temp_card: Some(false),
            card_type: Some(sonettobuf::card_info::CardType::Device as i32),
            hero_id: Some(0),
            status: Some(0),
            target_uid: Some(0),
            energy: Some(0),
            area_red_or_blue: Some(0),
            heat_id: Some(0),
            ..Default::default()
        }
    );
    assert!(manager.hand().is_empty());
}

#[test]
fn around_rank_change_records_success_and_rank_floor_failure_in_order() {
    let card = |uid, skill_id| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut manager = CardManager::new(vec![
        card(1, 100),
        card(2, 200),
        card(3, 300),
        card(4, 400),
        card(5, 30480222),
        card(6, 30990151),
        card(7, 31080121),
    ]);
    manager.rank_up.insert((5, 30480222), 30480223);
    manager.rank_up.insert((7, 31080121), 31080122);
    for _ in 0..7 {
        manager
            .execute_command(CardCommand::Play(CardPlay {
                origin: ORIGIN,
                hand_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
    }
    // Captured/replayed actions may already name the resolved skill, while the
    // queued card snapshot still owns the pre-preparation rank.
    manager.played[4].skill_id = 30480223;

    let changes = manager
        .execute_command(CardCommand::ChangeQueuedRanks {
            origin: ORIGIN,
            changes: vec![
                QueuedCardRankChange {
                    card_index: 5,
                    levels: 1,
                },
                QueuedCardRankChange {
                    card_index: 7,
                    levels: -1,
                },
            ],
        })
        .unwrap();

    assert_eq!(changes.rank_results.len(), 2);
    assert!(matches!(
        &changes.rank_results[0],
        CardRankResult::Changed(change)
            if change.card_index == 5 && change.card.skill_id == Some(30480223)
    ));
    assert!(matches!(
        &changes.rank_results[1],
        CardRankResult::Failed(failure)
            if failure.card_index == 7 && failure.card.skill_id == Some(31080121)
    ));
}
