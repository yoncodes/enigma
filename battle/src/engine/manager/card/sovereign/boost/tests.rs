use super::*;
use crate::engine::{
    manager::card::{CARD_PLAY_ORIGIN, CardChangeKind, CardCommand, CardInsertSovereign},
    packet::timeline,
    runtime::{
        change::BattleChange,
        record::{FrameItem, FrameOwner, FrameTrigger, SemanticFrame},
    },
};
use sonettobuf::{CardInfo, effect_type_enum::EffectType};

#[test]
fn initial_boost_keeps_identity() {
    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    for hand_size in [4, 6, 0] {
        let mut manager = CardManager::new(vec![normal.clone(); hand_size]);
        manager
            .execute_command(CardCommand::InsertSovereign(CardInsertSovereign {
                origin: CARD_PLAY_ORIGIN,
                target_uid: 10,
                skill_id: 31472131,
                hero_id: 3147,
                max_position: 9,
                team_type: 1,
            }))
            .unwrap();
        let initial_card = manager.hand()[hand_size].clone();
        let physical_id = manager.deck.hand_id(hand_size).unwrap();
        let command = CardCommand::InitialSovereignBoost(CardInitialSovereignBoost {
            origin: CARD_PLAY_ORIGIN,
            owner_uid: 10,
            index: hand_size,
            distance: 8,
            team_type: 1,
        });
        if hand_size == 4 {
            for invalid in [
                None,
                Some(r#"{"lock":true,"strengthen":{"1":1,"2":0,"3":0,"4":0}}"#),
                Some(r#"{"lock":true,"strengthen":{"1":0,"2":0,"3":0,"4":0},"unknown":0}"#),
                Some("malformed"),
            ] {
                let mut invalid_manager = manager.clone();
                if let Some(value) = invalid {
                    invalid_manager.deck.hand_mut()[hand_size].card_dataes[0].value =
                        Some(value.to_owned());
                } else {
                    invalid_manager.deck.hand_mut()[hand_size]
                        .card_dataes
                        .clear();
                }
                let before = invalid_manager.clone();
                assert_eq!(
                    invalid_manager.execute_command(command.clone()),
                    Err(CardCommandError::InvalidCommand)
                );
                assert_eq!(invalid_manager, before);
            }
        }
        let changes = manager.execute_command(command).unwrap();
        assert_eq!(changes.kind, CardChangeKind::InitialSovereignBoost);
        assert_eq!(manager.deck.hand_id(0), Some(physical_id));
        assert_eq!(&manager.hand()[1..], vec![normal.clone(); hand_size]);
        let mut expected = initial_card;
        expected.card_dataes[0].value =
            Some(r#"{"lock":true,"strengthen":{"1":4,"2":4,"3":4,"4":4}}"#.to_owned());
        assert_eq!(manager.hand()[0], expected);
        assert!(!manager.can_use_card(&expected));
        let boost = changes.sovereign_boost.as_ref().unwrap();
        assert_eq!(
            (boost.from_index, boost.to_index, boost.team_type),
            (hand_size, 0, 1)
        );
        assert_eq!(boost.card, expected);
        manager.resync_hand(Vec::new());
        let steps = timeline::project_for_version(
            &[SemanticFrame {
                owner: FrameOwner::Command,
                trigger: FrameTrigger::Active,
                items: vec![FrameItem::Change(Box::new(BattleChange::Card(Box::new(
                    changes,
                ))))],
            }],
            7,
        )
        .unwrap();
        let effects = &steps[0].act_effect;
        assert_eq!(effects.len(), 2);
        let moved = &effects[0];
        assert_eq!(moved.effect_type, Some(EffectType::Movecard as i32));
        assert_eq!(
            (moved.effect_num, moved.effect_num1),
            (Some(hand_size as i32 + 1), Some(1))
        );
        assert_eq!((moved.target_id, moved.team_type), (Some(0), Some(0)));
        let update = &effects[1];
        assert_eq!(update.effect_type, Some(EffectType::Updatecarddata as i32));
        assert_eq!((update.effect_num, update.effect_num1), (Some(1), Some(0)));
        assert_eq!((update.target_id, update.team_type), (Some(0), Some(1)));
        assert_eq!(update.card_info, Some(expected));
        assert!(effects.iter().all(|effect| effect.config_effect == Some(0)
            && effect.buff_act_id == Some(0)
            && effect.reserve_id == Some(0)));
    }
}
